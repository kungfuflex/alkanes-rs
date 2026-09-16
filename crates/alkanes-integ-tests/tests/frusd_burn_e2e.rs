//! Test: frUSD BurnAndBridge flow — burn frUSD tokens and create a bridge withdrawal record.
//!
//! Models the flow from subfrost-app/__tests__/devnet/e2e-frusd-bridge.test.ts

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_deploys_to_address, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{
    transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness,
};
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;
use prost::Message;

const AUTH_SLOT: u128 = 50000;
const TOKEN_SLOT: u128 = 50001;

#[test]
fn burn_and_bridge_frusd() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // ── Step 1: Deploy auth + frUSD ──
    println!("=== Step 1: Deploy auth + frUSD ===");
    let deploy1 = create_block_with_deploys(4, vec![
        DeployPair::new(
            fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_SLOT }, inputs: vec![0] },
        ),
    ]);
    runtime.index_block(&deploy1, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy1);

    let deploy2 = create_block_with_deploys(5, vec![
        DeployPair::new(
            fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack {
                target: AlkaneId { block: 3, tx: TOKEN_SLOT },
                inputs: vec![0, 4, AUTH_SLOT],
            },
        ),
    ]);
    runtime.index_block(&deploy2, 5)?;
    println!("Auth at [{AUTH_SLOT}], frUSD at [{TOKEN_SLOT}]");

    // ── Step 2: Mint frUSD (spend auth_outpoint so auth token → incoming_alkanes) ──
    println!("\n=== Step 2: Mint 10000 frUSD ===");
    let user_address = ADDRESS1();
    let mint_block = create_block_with_deploys_to_address(
        6,
        vec![DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 10000], // mint 10000
        })],
        auth_outpoint,     // spend the auth UTXO (auth token → contract)
        &user_address,     // minted frUSD goes to user address
    );
    runtime.index_block(&mint_block, 6)?;
    let mint_outpoint = last_tx_outpoint(&mint_block);

    // Check what token was minted
    let balances = query::get_balance_for_outpoint(&runtime, &mint_outpoint, 6)?;
    println!("Balances at mint outpoint:");
    let mut frusd_token_id = None;
    for (block, tx, bal) in &balances {
        println!("  [{block}:{tx}] = {bal}");
        if *bal > 0 && *block != 4 || (*block == 4 && *tx != AUTH_SLOT && *tx != TOKEN_SLOT) {
            frusd_token_id = Some((*block, *tx));
        }
    }
    println!("frUSD circulating token: {:?}", frusd_token_id);

    // ── Step 3: BurnAndBridge ──
    println!("\n=== Step 3: BurnAndBridge 5000 frUSD ===");
    // ETH address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    let eth_hi: u128 = 0xf39Fd6e51aad88F6F4ce6aB8827279cf;
    let eth_lo: u128 = 0xfFb92266;

    // Build TX that:
    // 1. Spends the mint outpoint (which holds frUSD)
    // 2. Calls BurnAndBridge (opcode 5) on frUSD token
    // 3. Routes frUSD to the protomessage via Runestone pointer
    let user_addr = get_address(&ADDRESS1());
    let mut block7 = create_block_with_coinbase_tx(7);

    let protostone = Protostone {
        message: Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![5, eth_hi, eth_lo], // BurnAndBridge(eth_addr)
        }.encipher(),
        protocol_tag: 1,
        burn: None,
        from: None,
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
    };

    let protostones = vec![protostone];
    let runestone_script = (ordinals::Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        // Pointer = 2 = first protomessage (2 real outputs: user + OP_RETURN)
        // This routes ALL unallocated runes to the contract as incoming_alkanes
        pointer: Some(2),
        protocol: protostones.encipher().ok(),
    }).encipher();

    let tx = bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: mint_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut { value: Amount::from_sat(100_000_000), script_pubkey: user_addr.script_pubkey() },
            TxOut { value: Amount::from_sat(0), script_pubkey: runestone_script },
        ],
    };
    block7.txdata.push(tx);
    runtime.index_block(&block7, 7)?;

    let burn_outpoint = last_tx_outpoint(&block7);
    println!("BurnAndBridge TX indexed");

    // ── Step 4: Check balances after burn ──
    let post_balances = query::get_balance_for_outpoint(&runtime, &burn_outpoint, 7)?;
    println!("\nPost-burn balances:");
    for (block, tx, bal) in &post_balances {
        println!("  [{block}:{tx}] = {bal}");
    }

    // ── Step 5: Check pending burns via simulate ──
    println!("\n=== Step 5: Check pending bridges ===");
    let cellpack = Cellpack {
        target: AlkaneId { block: 4, tx: TOKEN_SLOT },
        inputs: vec![6], // PendingBridges
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), 7)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;

    if !sim.error.is_empty() {
        println!("Simulate error: {}", sim.error);
    }
    if let Some(exec) = &sim.execution {
        println!("Pending bridges data: {} bytes", exec.data.len());
        if exec.data.len() >= 16 {
            let count = u128::from_le_bytes(exec.data[..16].try_into().unwrap());
            println!("Pending bridge count: {count}");
        }
    }

    println!("\n✓ BurnAndBridge test complete");
    Ok(())
}
