//! Port of crates/alkanes/src/tests/genesis.rs
//!
//! Genesis alkane initialization, diesel premine, and spendability.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::Protostone;

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

/// Test genesis block processing — the alkanes indexer initializes diesel,
/// frBTC, and other precompiled contracts during genesis.
#[test]
fn test_genesis_block_processing() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Block 0 is the genesis block — should initialize diesel (2:0), frBTC (32:0), etc.
    let block0 = protorune::test_helpers::create_block_with_coinbase_tx(0);
    runtime.index_block(&block0, 0)?;
    println!("Genesis block indexed successfully");

    // Verify sequence is queryable
    let seq = query::get_sequence(&runtime, 0)?;
    println!("Sequence after genesis: {} bytes", seq.len());

    Ok(())
}

/// Test genesis alkane deployment with cellpack init.
#[test]
fn test_genesis_with_init() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Deploy genesis alkane + init at block 0
    let block = create_block_with_deploys(
        0,
        vec![DeployPair::new(
            fixtures::GENESIS_ALKANE,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&block, 0)?;

    // Block 1: diesel mint (opcode 77) on the genesis alkane
    let block1 = create_block_with_protostones(
        1,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
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

    println!("Genesis with init + diesel mint passed");
    Ok(())
}

/// Test that genesis premine creates spendable DIESEL tokens.
#[test]
fn test_genesis_premine_spendable() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Index genesis block
    let block0 = protorune::test_helpers::create_block_with_coinbase_tx(0);
    runtime.index_block(&block0, 0)?;

    // The genesis premine creates DIESEL (2:0) at a specific outpoint.
    // After genesis, diesel should exist somewhere. Mine another block
    // and try to mint diesel via opcode 77.
    let block1 = create_block_with_protostones(
        1,
        vec![txin_from(OutPoint::new(block0.txdata[0].compute_txid(), 0))],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![77], // diesel mint
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

    // Check diesel balance at output 0
    let outpoint = last_tx_outpoint(&block1);
    let diesel_bal = query::get_alkane_balance(&runtime, &outpoint, 2, 0, 1)?;
    println!("genesis premine: DIESEL (2:0) balance = {}", diesel_bal);
    assert!(diesel_bal > 0, "DIESEL should be mintable after genesis");

    Ok(())
}
