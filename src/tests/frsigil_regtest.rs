//! Validates the `regtest_frsigil` build: the frSIGIL (32:1) auth token is
//! premined onto block 1's coinbase (a b8-controllable, bitcoin-spendable
//! regtest outpoint), and spending that coinbase authorizes `set_signer`
//! (opcode 1) on frBTC (32:0) — proving b8 can install its own signer while
//! the frSIGIL auth check stays enforced.

use crate::indexer::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_return_context, assert_revert_context, clear};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::blockdata::transaction::{OutPoint, Version};
use bitcoin::{Amount, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
use ordinals::Runestone;
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;

fn frbtc() -> AlkaneId {
    AlkaneId { block: 32, tx: 0 }
}

/// A set_signer tx spending `input` (which must carry frSIGIL): output 0 is
/// where frSIGIL is routed back (runestone + protostone pointer 0), output 1
/// is the new signer (signer_vout = 1, distinct from the pointer), output 2 is
/// the runestone OP_RETURN.
fn set_signer_tx(input: OutPoint, signer_vout: u128) -> Transaction {
    let protostone = vec![Protostone {
        message: Cellpack {
            target: frbtc(),
            inputs: vec![1, signer_vout],
        }
        .encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: 1,
    }];
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostone.encipher().ok(),
    })
    .encipher();
    let spk = get_address(&ADDRESS1()).script_pubkey();
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: input,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }],
        output: vec![
            TxOut { value: Amount::from_sat(546), script_pubkey: spk.clone() },
            TxOut { value: Amount::from_sat(546), script_pubkey: spk },
            TxOut { value: Amount::from_sat(0), script_pubkey: runestone },
        ],
    }
}

/// Spending the block-1 coinbase (which holds the deferred frSIGIL premine)
/// into a set_signer call is AUTHORIZED and returns successfully.
#[test]
fn test_regtest_frsigil_premine_authorizes_set_signer() -> Result<()> {
    clear();

    // Index block 1: genesis runs (GENESIS_BLOCK = 0), frBTC/frSIGIL deploy,
    // and the deferred hook premines frSIGIL (32:1) onto this block's coinbase.
    let block1 = create_block_with_coinbase_tx(1);
    let coinbase_outpoint = OutPoint {
        txid: block1.txdata[0].compute_txid(),
        vout: 0,
    };
    index_block(&block1, 1)?;

    // Spend the frSIGIL-bearing coinbase into set_signer (signer_vout = 1).
    let tx = set_signer_tx(coinbase_outpoint, 1);
    let mut block2 = create_block_with_coinbase_tx(2);
    block2.txdata.push(tx.clone());
    index_block(&block2, 2)?;

    // The protomessage returns (auth passed; signer installed) — vout = the
    // protomessage shadow output (tx.output.len() + protostone index = 3).
    let shadow = OutPoint {
        txid: tx.compute_txid(),
        vout: 4,
    };
    assert_return_context(&shadow, |_trace| Ok(()))?;
    Ok(())
}

/// Control: without frSIGIL (spending an empty outpoint), set_signer reverts
/// with the auth error — the frSIGIL requirement is preserved on this build.
#[test]
fn test_regtest_frsigil_set_signer_still_requires_auth() -> Result<()> {
    clear();
    let block1 = create_block_with_coinbase_tx(1);
    index_block(&block1, 1)?;

    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::default(),
        vec![Cellpack {
            target: frbtc(),
            inputs: vec![1, 0],
        }],
        OutPoint::default(),
        false,
    );
    let mut block2 = create_block_with_coinbase_tx(2);
    block2.txdata.push(tx.clone());
    index_block(&block2, 2)?;

    let shadow = OutPoint {
        txid: tx.compute_txid(),
        vout: 3,
    };
    assert_revert_context(&shadow, "Auth token is not in incoming alkanes")?;
    Ok(())
}
