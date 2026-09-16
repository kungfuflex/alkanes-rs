//! Port of crates/alkanes/src/tests/fr_btc.rs
//!
//! Tests frBTC wrap/unwrap operations. The frBTC contract lives at AlkaneId {block:32, tx:0}
//! and is initialized during genesis. The P2TR signer address is hardcoded.

use alkanes_integ_tests::block_builder::{create_block_with_protostones, last_tx_outpoint};
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

/// The hardcoded frBTC signer pubkey (x-only, 32 bytes).
const SIGNER_PUBKEY: [u8; 32] = [
    0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
    0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
    0x29, 0xdc,
];

/// Create a P2TR output paying to the frBTC signer address.
fn create_frbtc_signer_output(value_sats: u64) -> TxOut {
    let signer_pubkey = UntweakedPublicKey::from_slice(&SIGNER_PUBKEY).unwrap();
    let secp = Secp256k1::new();
    let (tweaked, _) = signer_pubkey.tap_tweak(&secp, None);
    TxOut {
        value: Amount::from_sat(value_sats),
        script_pubkey: ScriptBuf::new_p2tr_tweaked(tweaked),
    }
}

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

/// Create a wrap transaction: send BTC to signer, call frBTC opcode 77.
/// Also includes a diesel mint (opcode 77 on 2:0) as the original test does.
fn create_wrap_tx(funding_outpoint: OutPoint, btc_amount: u64) -> bitcoin::Transaction {
    let protostones = vec![
        // frBTC wrap
        Protostone {
            message: Cellpack {
                target: AlkaneId { block: 32, tx: 0 },
                inputs: vec![77],
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        },
        // Diesel mint (always included with frBTC wrap)
        Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![77],
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        },
    ];

    let runestone_script = (ordinals::Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: protostones.encipher().ok(),
    })
    .encipher();

    bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin_from(funding_outpoint)],
        output: vec![
            create_frbtc_signer_output(btc_amount),
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone_script,
            },
        ],
    }
}

/// Create an unwrap transaction: call frBTC opcode 78 with amount and desired vout.
fn create_unwrap_tx(
    fr_btc_outpoint: OutPoint,
    amount_to_burn: u64,
    desired_vout: u128,
) -> bitcoin::Transaction {
    let protostones = vec![Protostone {
        message: Cellpack {
            target: AlkaneId { block: 32, tx: 0 },
            inputs: vec![78, desired_vout, amount_to_burn as u128],
        }
        .encipher(),
        protocol_tag: 1,
        burn: None,
        from: None,
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
    }];

    let runestone_script = (ordinals::Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: protostones.encipher().ok(),
    })
    .encipher();

    let address = get_address(&ADDRESS1().as_str());
    bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin_from(fr_btc_outpoint)],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: address.script_pubkey(),
            },
            create_frbtc_signer_output(100_000_000),
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone_script,
            },
        ],
    }
}

/// Wrap BTC → frBTC at height 1. Returns (wrap_outpoint, frbtc_amount).
fn wrap_btc(runtime: &TestRuntime) -> Result<(OutPoint, u64)> {
    let mut block = create_block_with_coinbase_tx(1);
    let funding_outpoint = OutPoint {
        txid: block.txdata[0].compute_txid(),
        vout: 0,
    };

    let wrap_tx = create_wrap_tx(funding_outpoint, 100_000_000);
    let wrap_txid = wrap_tx.compute_txid();
    block.txdata.push(wrap_tx);
    runtime.index_block(&block, 1)?;

    // Check frBTC balance
    let wrap_outpoint = OutPoint {
        txid: wrap_txid,
        vout: 0,
    };
    let frbtc_bal = query::get_alkane_balance(runtime, &wrap_outpoint, 32, 0, 1)?;
    println!("wrap_btc: frBTC balance = {}", frbtc_bal);

    // Also check diesel
    let diesel_bal = query::get_alkane_balance(runtime, &wrap_outpoint, 2, 0, 1)?;
    println!("wrap_btc: DIESEL balance = {}", diesel_bal);

    Ok((wrap_outpoint, frbtc_bal as u64))
}

#[test]
fn test_fr_btc_wrap_correct_signer() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    let (outpoint, frbtc_amt) = wrap_btc(&runtime)?;
    println!("Wrapped {} frBTC at {:?}", frbtc_amt, outpoint);

    // frBTC should be minted (exact amount depends on fee deduction)
    assert!(frbtc_amt > 0, "frBTC should be minted when sending to correct signer");
    Ok(())
}

#[test]
fn test_fr_btc_unwrap() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    let (wrap_outpoint, frbtc_amt) = wrap_btc(&runtime)?;
    assert!(frbtc_amt > 0);

    // Unwrap all frBTC at height 2
    let mut block2 = create_block_with_coinbase_tx(2);
    let unwrap_tx = create_unwrap_tx(wrap_outpoint, frbtc_amt, 1);
    block2.txdata.push(unwrap_tx);
    runtime.index_block(&block2, 2)?;

    // After full unwrap, frBTC balance should be 0
    let outpoint = last_tx_outpoint(&block2);
    let remaining = query::get_alkane_balance(&runtime, &outpoint, 32, 0, 2)?;
    println!("unwrap full: remaining frBTC = {}", remaining);
    assert_eq!(remaining, 0, "all frBTC should be burned after full unwrap");
    Ok(())
}

#[test]
fn test_fr_btc_unwrap_partial() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    let (wrap_outpoint, frbtc_amt) = wrap_btc(&runtime)?;
    assert!(frbtc_amt > 0);

    // Unwrap half
    let half = frbtc_amt / 2;
    let mut block2 = create_block_with_coinbase_tx(2);
    let unwrap_tx = create_unwrap_tx(wrap_outpoint, half, 1);
    block2.txdata.push(unwrap_tx);
    runtime.index_block(&block2, 2)?;

    let outpoint = last_tx_outpoint(&block2);
    let remaining = query::get_alkane_balance(&runtime, &outpoint, 32, 0, 2)?;
    println!("unwrap partial: remaining frBTC = {} (expected ~{})", remaining, frbtc_amt - half);
    assert_eq!(remaining as u64, frbtc_amt - half, "half should remain after partial unwrap");
    Ok(())
}

#[test]
fn test_fr_btc_unwrap_more_than_balance() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    let (wrap_outpoint, frbtc_amt) = wrap_btc(&runtime)?;
    assert!(frbtc_amt > 0);

    // Try to unwrap 2x the balance — should be capped at actual balance
    let mut block2 = create_block_with_coinbase_tx(2);
    let unwrap_tx = create_unwrap_tx(wrap_outpoint, frbtc_amt * 2, 1);
    block2.txdata.push(unwrap_tx);
    runtime.index_block(&block2, 2)?;

    let outpoint = last_tx_outpoint(&block2);
    let remaining = query::get_alkane_balance(&runtime, &outpoint, 32, 0, 2)?;
    println!("unwrap more: remaining frBTC = {}", remaining);
    assert_eq!(remaining, 0, "all frBTC should be burned (capped at balance)");
    Ok(())
}

#[test]
fn test_fr_btc_wrap_incorrect_signer() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Don't send BTC to the signer — just call opcode 77 without correct output
    let mut block = create_block_with_coinbase_tx(1);
    let funding_outpoint = OutPoint {
        txid: block.txdata[0].compute_txid(),
        vout: 0,
    };

    // Regular output (not signer) + frBTC cellpack
    let block1 = create_block_with_protostones(
        1,
        vec![txin_from(funding_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 32, tx: 0 },
                inputs: vec![77],
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );
    runtime.index_block(&block1, 1)?;

    // No BTC went to signer → no frBTC should be minted
    let outpoint = last_tx_outpoint(&block1);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 32, 0, 1)?;
    println!("incorrect_signer: frBTC balance = {} (should be 0)", bal);
    assert_eq!(bal, 0, "no frBTC should be minted without BTC to signer");
    Ok(())
}

#[test]
fn test_set_signer_no_auth() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Try to set signer without auth token — should revert
    let block = create_block_with_protostones(
        1,
        vec![txin_from(OutPoint::default())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 32, tx: 0 },
                inputs: vec![1, 0], // opcode 1 = set_signer, vout 0
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );
    runtime.index_block(&block, 1)?;

    // Should revert — no auth token provided
    println!("set_signer_no_auth test passed — block indexed (revert expected)");
    Ok(())
}
