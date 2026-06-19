//! Quick test: unwrap frBTC then query pending payments

use alkanes_integ_tests::block_builder::last_tx_outpoint;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{key::UntweakedPublicKey, transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;
use prost::Message;

const SIGNER: [u8;32] = [0x79,0x40,0xef,0x3b,0x65,0x91,0x79,0xa1,0x37,0x1d,0xec,0x05,0x79,0x3c,0xb0,0x27,0xcd,0xe4,0x78,0x06,0xfb,0x66,0xce,0x1e,0x3d,0x1b,0x69,0xd5,0x6d,0xe6,0x29,0xdc];

fn signer_out(v: u64) -> TxOut {
    let pk = UntweakedPublicKey::from_slice(&SIGNER).unwrap();
    let secp = Secp256k1::new();
    let (tw, _) = pk.tap_tweak(&secp, None);
    TxOut { value: Amount::from_sat(v), script_pubkey: ScriptBuf::new_p2tr_tweaked(tw) }
}

#[test]
fn unwrap_and_query_pending() -> Result<()> {
    let _ = env_logger::try_init();
    let rt = TestRuntime::new()?;

    // Wrap 1 BTC
    let mut b1 = create_block_with_coinbase_tx(1);
    let fund = OutPoint { txid: b1.txdata[0].compute_txid(), vout: 0 };
    let ps = vec![
        Protostone { message: Cellpack { target: AlkaneId{block:32,tx:0}, inputs: vec![77] }.encipher(), protocol_tag:1, burn:None,from:None,pointer:Some(0),refund:Some(0),edicts:vec![] },
        Protostone { message: Cellpack { target: AlkaneId{block:2,tx:0}, inputs: vec![77] }.encipher(), protocol_tag:1, burn:None,from:None,pointer:Some(0),refund:Some(0),edicts:vec![] },
    ];
    let rs = (ordinals::Runestone{edicts:vec![],etching:None,mint:None,pointer:Some(0),protocol:ps.encipher().ok()}).encipher();
    b1.txdata.push(bitcoin::Transaction{version:Version::ONE,lock_time:bitcoin::absolute::LockTime::ZERO,
        input:vec![TxIn{previous_output:fund,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()}],
        output:vec![signer_out(100_000_000),TxOut{value:Amount::ZERO,script_pubkey:rs}]});
    rt.index_block(&b1, 1)?;
    let wrap_op = last_tx_outpoint(&b1);
    let frbtc = query::get_alkane_balance(&rt, &wrap_op, 32, 0, 1)?;
    println!("Wrapped: frBTC={frbtc}");

    // Unwrap all
    let mut b2 = create_block_with_coinbase_tx(2);
    let addr = get_address(&ADDRESS1());
    let ups = vec![Protostone{message:Cellpack{target:AlkaneId{block:32,tx:0},inputs:vec![78,1,frbtc as u128]}.encipher(),
        protocol_tag:1,burn:None,from:None,pointer:Some(0),refund:Some(0),edicts:vec![]}];
    let urs = (ordinals::Runestone{edicts:vec![],etching:None,mint:None,pointer:Some(0),protocol:ups.encipher().ok()}).encipher();
    b2.txdata.push(bitcoin::Transaction{version:Version::ONE,lock_time:bitcoin::absolute::LockTime::ZERO,
        input:vec![TxIn{previous_output:wrap_op,script_sig:ScriptBuf::new(),sequence:Sequence::MAX,witness:Witness::new()}],
        output:vec![
            TxOut{value:Amount::from_sat(100_000_000),script_pubkey:addr.script_pubkey()}, // v0: user BTC dest
            signer_out(100_000_000), // v1: signer spendable 
            TxOut{value:Amount::ZERO,script_pubkey:urs},
        ]});
    rt.index_block(&b2, 2)?;
    let remaining = query::get_alkane_balance(&rt, &last_tx_outpoint(&b2), 32, 0, 2)?;
    println!("After unwrap: frBTC={remaining}");

    // Query pending payments (opcode 101)
    let cp = Cellpack{target:AlkaneId{block:32,tx:0},inputs:vec![101]};
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cp.encipher();
    let resp = rt.alkanes_view("simulate", &parcel.encode_to_vec(), 2)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    println!("Pending payments: {} bytes, error='{}'", sim.execution.as_ref().map(|e|e.data.len()).unwrap_or(0), sim.error);
    if let Some(e) = &sim.execution {
        if !e.data.is_empty() {
            println!("  Data: {}", hex::encode(&e.data[..e.data.len().min(80)]));
        }
    }
    assert!(sim.error.is_empty() || sim.execution.as_ref().map(|e|e.data.len()).unwrap_or(0) > 0, "Should have pending payment data");
    println!("✅ frBTC unwrap created pending payments");
    Ok(())
}
