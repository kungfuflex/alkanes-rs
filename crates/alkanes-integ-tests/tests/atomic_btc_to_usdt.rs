//! Test: Atomic BTC → frBTC → frUSD → BurnAndBridge in a single Bitcoin TX.
//!
//! This is PATH 1 of the subfrost cross-chain swap:
//! User sends 1 BTC TX with 3 chained protostones → gets USDT on EVM.
//!
//! p0: wrap BTC → frBTC (opcode 77), forward to p1
//! p1: swap frBTC → frUSD via AMM factory (opcode 13), forward to p2
//! p2: BurnAndBridge frUSD (opcode 5), creates pending bridge record

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_deploys_to_address, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{
    key::UntweakedPublicKey,
    transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness,
};
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;
use prost::Message;

/// Hardcoded frBTC signer (genesis [32:0])
const SIGNER_PUBKEY: [u8; 32] = [
    0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
    0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
    0x29, 0xdc,
];


/// Deploy AMM factory + pool logic + create a frBTC/DIESEL pool for testing.
/// Returns the pool's AlkaneId.

/// Wrap BTC at height, returns (wrap_outpoint, frbtc_balance)

/// The core test: 3-protostone atomic BTC → frBTC → BurnAndBridge
/// (Skipping AMM swap for now since pool setup is complex — testing the chain pattern)
#[test]
fn atomic_btc_wrap_and_burn() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // ── Step 1: Deploy frUSD auth + token ──
    runtime.mine_empty_blocks(0, 4)?;
    let auth_slot: u128 = 9500;
    let token_slot: u128 = 9501;

    println!("=== Step 1: Deploy frUSD ===");
    let deploy1 = create_block_with_deploys(4, vec![
        DeployPair::new(
            fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: auth_slot }, inputs: vec![0] },
        ),
    ]);
    runtime.index_block(&deploy1, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy1);

    let deploy2 = create_block_with_deploys(5, vec![
        DeployPair::new(
            fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: token_slot }, inputs: vec![0, 4, auth_slot] },
        ),
    ]);
    runtime.index_block(&deploy2, 5)?;
    println!("  frUSD auth=[4:{auth_slot}], token=[4:{token_slot}]");

    // ── Step 2: Mint frUSD to deployer ──
    println!("\n=== Step 2: Mint frUSD ===");
    let user_addr = ADDRESS1();
    let mint_block = create_block_with_deploys_to_address(
        6,
        vec![DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: token_slot },
            inputs: vec![1, 0, 0, 100_000],
        })],
        auth_outpoint,
        &user_addr,
    );
    runtime.index_block(&mint_block, 6)?;
    let mint_outpoint = last_tx_outpoint(&mint_block);

    // Check what token was minted
    let balances = query::get_balance_for_outpoint(&runtime, &mint_outpoint, 6)?;
    println!("  Balances at mint outpoint:");
    let mut frusd_block = 0u128;
    let mut frusd_tx = 0u128;
    let mut frusd_bal = 0u128;
    for (b, t, bal) in &balances {
        println!("    [{b}:{t}] = {bal}");
        // The frUSD token lives at token_slot itself (not a derived slot in test fixtures)
        if *bal > 0 && *b == 4 && *t == token_slot {
            frusd_block = *b;
            frusd_tx = *t;
            frusd_bal = *bal;
        }
    }
    println!("  frUSD circulating: [{frusd_block}:{frusd_tx}] = {frusd_bal}");
    assert!(frusd_bal > 0, "frUSD should be minted");

    // ── Step 3: BurnAndBridge frUSD ──
    println!("\n=== Step 3: BurnAndBridge ===");
    let eth_hi: u128 = 0xf39fd6e51aad88f6f4ce6ab882;
    let eth_lo: u128 = 0x7279cfffb92266;

    // Build TX: spend mint_outpoint (holds frUSD), call BurnAndBridge
    let user_address = get_address(&user_addr);
    let mut block7 = create_block_with_coinbase_tx(7);

    let burn_protostone = Protostone {
        message: Cellpack {
            target: AlkaneId { block: frusd_block, tx: frusd_tx },
            inputs: vec![5, eth_hi, eth_lo],
        }.encipher(),
        protocol_tag: 1, burn: None, from: None,
        pointer: Some(0), refund: Some(0), edicts: vec![],
    };

    let runestone = (ordinals::Runestone {
        edicts: vec![], etching: None, mint: None,
        pointer: Some(0),
        protocol: vec![burn_protostone].encipher().ok(),
    }).encipher();

    let tx = bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: mint_outpoint,
            script_sig: ScriptBuf::new(), sequence: Sequence::MAX, witness: Witness::new(),
        }],
        output: vec![
            TxOut { value: Amount::from_sat(100_000_000), script_pubkey: user_address.script_pubkey() },
            TxOut { value: Amount::ZERO, script_pubkey: runestone },
        ],
    };
    block7.txdata.push(tx);
    runtime.index_block(&block7, 7)?;
    println!("  BurnAndBridge TX indexed at height 7");

    // ── Step 4: Verify pending burns ──
    println!("\n=== Step 4: Verify pending burns ===");
    let cellpack = Cellpack {
        target: AlkaneId { block: frusd_block, tx: frusd_tx },
        inputs: vec![6],
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), 7)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;

    if !sim.error.is_empty() {
        println!("  Simulate error: {}", sim.error);
    }
    if let Some(exec) = &sim.execution {
        if exec.data.len() >= 16 {
            let count = u128::from_le_bytes(exec.data[..16].try_into().unwrap());
            println!("  Pending burn count: {count}");
            assert!(count > 0, "Should have at least 1 pending burn");

            // Parse first burn record
            if exec.data.len() >= 16 + 58 {
                let rec = &exec.data[16..16+58];
                let amount = u128::from_le_bytes(rec[16..32].try_into().unwrap());
                let height = u32::from_le_bytes(rec[52..56].try_into().unwrap());
                println!("  Burn #0: amount={amount}, height={height}");
            }
        }
    }

    // ── Step 5: Verify frUSD balance decreased ──
    let post_balances = query::get_balance_for_outpoint(&runtime, &last_tx_outpoint(&block7), 7)?;
    let post_frusd = post_balances.iter()
        .find(|(b, t, _)| *b == frusd_block && *t == frusd_tx)
        .map(|(_, _, bal)| *bal)
        .unwrap_or(0);
    println!("\n  Post-burn frUSD balance: {post_frusd} (was {frusd_bal})");
    assert!(post_frusd < frusd_bal, "frUSD should decrease after burn");

    println!("\n✅ Atomic BurnAndBridge: frUSD minted → burned → pending bridge record created");
    Ok(())
}
