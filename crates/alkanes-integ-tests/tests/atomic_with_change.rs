//! Test: 3-protostone chain WITH a change output (matching CLI structure)

use alkanes_integ_tests::block_builder::*;
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness,
    key::{TapTweak, UntweakedPublicKey}, secp256k1::Secp256k1};
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;
use prost::Message;

const AUTH_SLOT: u128 = 8700;
const TOKEN_SLOT: u128 = 8701;
const SYNTH_SLOT: u128 = 8702;
const SIGNER: [u8;32] = [0x79,0x40,0xef,0x3b,0x65,0x91,0x79,0xa1,0x37,0x1d,0xec,0x05,0x79,0x3c,0xb0,0x27,0xcd,0xe4,0x78,0x06,0xfb,0x66,0xce,0x1e,0x3d,0x1b,0x69,0xd5,0x6d,0xe6,0x29,0xdc];

fn signer_out(v: u64) -> TxOut {
    let pk = UntweakedPublicKey::from_slice(&SIGNER).unwrap();
    let secp = Secp256k1::new();
    let (tw, _) = pk.tap_tweak(&secp, None);
    TxOut { value: Amount::from_sat(v), script_pubkey: ScriptBuf::new_p2tr_tweaked(tw) }
}

#[test]
fn atomic_3chain_with_change_output() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;
    let user_addr = ADDRESS1();
    let user_address = get_address(&user_addr);

    // Setup: same as e2e_btc_to_usdt_full::setup_pool
    let d1 = create_block_with_deploys(4, vec![
        DeployPair::new(fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_SLOT }, inputs: vec![0] }),
    ]);
    runtime.index_block(&d1, 4)?;
    let auth_op = last_tx_outpoint(&d1);

    let d2 = create_block_with_deploys(5, vec![
        DeployPair::new(fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: TOKEN_SLOT }, inputs: vec![0, 4, AUTH_SLOT] }),
    ]);
    runtime.index_block(&d2, 5)?;

    let mint = create_block_with_deploys_to_address(6,
        vec![DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 50_000_000],
        })], auth_op, &user_addr);
    runtime.index_block(&mint, 6)?;
    let mint_op = last_tx_outpoint(&mint);

    let mut b7 = create_block_with_coinbase_tx(7);
    let fund = OutPoint { txid: b7.txdata[0].compute_txid(), vout: 0 };
    let ps7 = vec![
        Protostone { message: Cellpack { target: AlkaneId{block:32,tx:0}, inputs: vec![77] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(0), refund:Some(0), edicts:vec![] },
        Protostone { message: Cellpack { target: AlkaneId{block:2,tx:0}, inputs: vec![77] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(0), refund:Some(0), edicts:vec![] },
    ];
    let rs7 = (ordinals::Runestone{edicts:vec![],etching:None,mint:None,pointer:Some(0),protocol:ps7.encipher().ok()}).encipher();
    b7.txdata.push(bitcoin::Transaction{version:Version::ONE,lock_time:bitcoin::absolute::LockTime::ZERO,
        input:vec![TxIn{previous_output:fund,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()}],
        output:vec![signer_out(100_000_000),TxOut{value:Amount::ZERO,script_pubkey:rs7}]});
    runtime.index_block(&b7, 7)?;
    let wrap_op = last_tx_outpoint(&b7);

    let synth = create_block_with_deploys(8, vec![
        DeployPair::new(fixtures::SYNTH_POOL.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: SYNTH_SLOT },
                inputs: vec![0, 4, TOKEN_SLOT, 32, 0, 100, 4000000, 5000000000, 4, AUTH_SLOT] }),
    ]);
    runtime.index_block(&synth, 8)?;

    let liq_ps = vec![Protostone {
        message: Cellpack { target: AlkaneId { block: 4, tx: SYNTH_SLOT }, inputs: vec![1] }.encipher(),
        protocol_tag: 1, burn: None, from: None, pointer: Some(0), refund: Some(0), edicts: vec![],
    }];
    let liq_rs = (ordinals::Runestone{edicts:vec![],etching:None,mint:None,pointer:Some(0),protocol:liq_ps.encipher().ok()}).encipher();
    let mut b9 = create_block_with_coinbase_tx(9);
    b9.txdata.push(bitcoin::Transaction{version:Version::ONE,lock_time:bitcoin::absolute::LockTime::ZERO,
        input:vec![
            TxIn{previous_output:mint_op,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()},
            TxIn{previous_output:wrap_op,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()},
        ],
        output:vec![
            TxOut{value:Amount::from_sat(100_000_000),script_pubkey:user_address.script_pubkey()},
            TxOut{value:Amount::ZERO,script_pubkey:liq_rs},
        ]});
    runtime.index_block(&b9, 9)?;
    println!("Pool seeded");

    // NOW: 3-protostone chain WITH change output (matching CLI layout)
    // Outputs: [v0=signer, v1=change(user), OP_RETURN]
    // tx.output.len() = 3
    // p0=4, p1=5, p2=6
    let mut b10 = create_block_with_coinbase_tx(10);
    let fund2 = OutPoint { txid: b10.txdata[0].compute_txid(), vout: 0 };
    let eth_hi: u128 = 0xf39fd6e51aad88f6f4ce6ab882;
    let eth_lo: u128 = 0x7279cfffb92266;

    let protostones = vec![
        // p0: wrap → pointer to p1 (vout 5)
        Protostone {
            message: Cellpack { target: AlkaneId{block:32,tx:0}, inputs: vec![77] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(5), refund:Some(0), edicts:vec![],
        },
        // p1: synth swap → pointer to p2 (vout 6)
        Protostone {
            message: Cellpack { target: AlkaneId{block:4,tx:SYNTH_SLOT}, inputs: vec![5] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(6), refund:Some(0), edicts:vec![],
        },
        // p2: BurnAndBridge → pointer to v0
        Protostone {
            message: Cellpack { target: AlkaneId{block:4,tx:TOKEN_SLOT}, inputs: vec![5, eth_hi, eth_lo] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(0), refund:Some(0), edicts:vec![],
        },
        // p3: DIESEL mint
        Protostone {
            message: Cellpack { target: AlkaneId{block:2,tx:0}, inputs: vec![77] }.encipher(),
            protocol_tag:1, burn:None, from:None, pointer:Some(0), refund:Some(0), edicts:vec![],
        },
    ];

    let rs = (ordinals::Runestone{edicts:vec![],etching:None,mint:None,pointer:Some(0),
        protocol:protostones.encipher().ok()}).encipher();

    // 3 outputs: signer, change, OP_RETURN (matching CLI layout)
    b10.txdata.push(bitcoin::Transaction{version:Version::ONE,lock_time:bitcoin::absolute::LockTime::ZERO,
        input:vec![TxIn{previous_output:fund2,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()}],
        output:vec![
            signer_out(50_000_000),       // v0: signer (BTC for wrap)
            TxOut{value:Amount::from_sat(49_000_000),script_pubkey:user_address.script_pubkey()}, // v1: change
            TxOut{value:Amount::ZERO,script_pubkey:rs},  // v2: OP_RETURN
        ]});
    runtime.index_block(&b10, 10)?;
    let atomic_op = last_tx_outpoint(&b10);

    println!("\n=== 3-protostone with CHANGE OUTPUT (CLI layout) ===");
    let bals = query::get_balance_for_outpoint(&runtime, &atomic_op, 10)?;
    println!("Output balances:");
    for (b, t, bal) in &bals {
        println!("  [{b}:{t}] = {bal}");
    }

    // Check burns
    let cp = Cellpack{target:AlkaneId{block:4,tx:TOKEN_SLOT},inputs:vec![6]};
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cp.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), 10)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    let burn_count = if let Some(exec) = &sim.execution {
        if exec.data.len() >= 16 {
            u128::from_le_bytes(exec.data[..16].try_into().unwrap())
        } else { 0 }
    } else { 0 };
    println!("Pending burns: {burn_count}");

    if burn_count > 0 {
        println!("\n✅ 3-protostone chain WITH change output WORKS!");
    } else {
        println!("\n❌ Failed with change output. The CLI layout breaks the chain.");
    }
    Ok(())
}
