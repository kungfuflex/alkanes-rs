//! Port of crates/alkanes/src/tests/crash.rs
//!
//! Tests that owned token mint operation doesn't crash the indexer.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::Protostone;

const AUTH_TOKEN_FACTORY_ID: u128 = 0xffed;

#[test]
fn test_owned_token_mint_crash() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy auth factory + owned token with init
    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::AUTH_TOKEN,
                Cellpack {
                    target: AlkaneId { block: 3, tx: AUTH_TOKEN_FACTORY_ID },
                    inputs: vec![100],
                },
            ),
            DeployPair::new(
                fixtures::OWNED_TOKEN,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![0, 1, 1000], // init with 1000 tokens
                },
            ),
        ],
    );
    runtime.index_block(&block, 4)?;

    // Verify initial balances
    let outpoint = last_tx_outpoint(&block);
    let owned_bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    println!("crash test: initial owned token balance = {}", owned_bal);

    // Now try to mint more — this should not crash the indexer
    let block5 = create_block_with_protostones(
        5,
        vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![77, 500], // mint 500 more
            }.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );
    runtime.index_block(&block5, 5)?;

    println!("crash test passed — no crash during owned token mint");
    Ok(())
}

#[test]
fn test_owned_token_init_and_verify() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::AUTH_TOKEN,
                Cellpack {
                    target: AlkaneId { block: 3, tx: AUTH_TOKEN_FACTORY_ID },
                    inputs: vec![100],
                },
            ),
            DeployPair::new(
                fixtures::OWNED_TOKEN,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![0, 1, 1000],
                },
            ),
        ],
    );
    runtime.index_block(&block, 4)?;

    let outpoint = last_tx_outpoint(&block);
    let owned_bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    let auth_bal = query::get_alkane_balance(&runtime, &outpoint, 2, 2, 4)?;
    println!("owned_token init: owned={}, auth={}", owned_bal, auth_bal);
    assert_eq!(owned_bal, 1000, "should have 1000 owned tokens");
    assert_eq!(auth_bal, 1, "should have 1 auth token");

    Ok(())
}
