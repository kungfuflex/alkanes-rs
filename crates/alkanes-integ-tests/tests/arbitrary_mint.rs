//! Port of crates/alkanes/src/tests/arbitrary_alkane_mint.rs
//!
//! Tests overflow, underflow, extcall mint, delegatecall mint, runtime duplication.

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::Protostone;

/// Helper: deploy test contract and mint tokens in one block.
fn deploy_and_mint(runtime: &TestRuntime, height: u32, amount: u128) -> Result<bitcoin::Block> {
    let block = create_block_with_deploys(
        height,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, amount],
            },
        )],
    );
    runtime.index_block(&block, height)?;
    Ok(block)
}

/// Helper: create a TxIn from an outpoint.
fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

#[test]
fn test_mint_underflow() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Mint with target block:2, tx:0 — but contract is at 2:1, so 2:0 doesn't exist
    // This should cause underflow
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 0, 1_000_000],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    let outpoint = last_tx_outpoint(&block);
    let bal_2_0 = query::get_alkane_balance(&runtime, &outpoint, 2, 0, 4)?;
    let bal_2_1 = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    println!("underflow test: 2:0={}, 2:1={}", bal_2_0, bal_2_1);
    assert_eq!(bal_2_0, 0);
    assert_eq!(bal_2_1, 0);
    Ok(())
}

#[test]
fn test_mint_overflow() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // First mint u128::MAX tokens
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, u128::MAX],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    // Second mint u128::MAX — should overflow and revert, tokens refunded
    let prev_outpoint = last_tx_outpoint(&block);
    let block2 = create_block_with_protostones(
        5,
        vec![txin_from(prev_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![30, 2, 1, u128::MAX],
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
    runtime.index_block(&block2, 5)?;

    // The first mint's tokens should be refunded to the output
    let outpoint = last_tx_outpoint(&block2);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!("overflow test: 2:1 balance = {} (should be u128::MAX)", bal);
    assert_eq!(bal, u128::MAX, "tokens should be refunded after overflow");
    Ok(())
}

#[test]
fn test_extcall_mint() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy with opcode 50 (init)
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![50],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    // Extcall (opcode 31) into self to mint via underflow path — should revert
    let block2 = create_block_with_protostones(
        5,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![31, 2, 1, 4, 30, 2, 0, 1_000_000],
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
    runtime.index_block(&block2, 5)?;

    let outpoint = last_tx_outpoint(&block2);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!("extcall_mint: 2:1 balance = {} (should be 0, reverted)", bal);
    assert_eq!(bal, 0);
    Ok(())
}

#[test]
fn test_delegatecall_mint() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![50],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    // Delegatecall (opcode 32) — should also revert on underflow
    let block2 = create_block_with_protostones(
        5,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![32, 2, 1, 4, 30, 2, 0, 1_000_000],
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
    runtime.index_block(&block2, 5)?;

    let outpoint = last_tx_outpoint(&block2);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!(
        "delegatecall_mint: 2:1 balance = {} (should be 0, reverted)",
        bal
    );
    assert_eq!(bal, 0);
    Ok(())
}

#[test]
fn test_multiple_extcall_err_and_good() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![50],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    // Batch extcall (opcode 34) — two extcalls, both should fail
    let block2 = create_block_with_protostones(
        5,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![34, 2, 1, 4, 30, 2, 0, 1_000_000, 2, 1, 4, 30, 2, 1, 1_000_000],
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
    runtime.index_block(&block2, 5)?;

    let outpoint = last_tx_outpoint(&block2);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!("batch_extcall: 2:1 balance = {} (should be 0)", bal);
    assert_eq!(bal, 0);
    Ok(())
}

#[test]
fn test_transfer_runtime() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Same as factory test: 4 cellpacks in one block
    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::TEST_CONTRACT,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![30, 2, 1, 1_000_000],
                },
            ),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![3], // send to runtime
            }),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 5, tx: 1 },
                inputs: vec![50], // create factory copy
            }),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 2 },
                inputs: vec![30, 2, 1, 1_000_000], // steal attempt
            }),
        ],
    );
    runtime.index_block(&block, 4)?;

    let outpoint = last_tx_outpoint(&block);
    let bal_2_1 = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    let bal_2_2 = query::get_alkane_balance(&runtime, &outpoint, 2, 2, 4)?;
    println!("transfer_runtime: 2:1={}, 2:2={}", bal_2_1, bal_2_2);

    // Steal should revert — 2:2 has no balance to mint from
    assert_eq!(bal_2_2, 0, "steal attempt should have 0 balance");
    Ok(())
}
