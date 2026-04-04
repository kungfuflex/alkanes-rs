//! Port of crates/alkanes/src/tests/memory_security_tests.rs

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

#[test]
fn test_integer_overflow_in_memory_operations() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Malformed cellpack with u128::MAX — should revert, not crash
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

    let outpoint = last_tx_outpoint(&block);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    println!("overflow test: 2:1 balance = {}", bal);
    // Should mint u128::MAX tokens (valid — overflow is on SECOND mint)
    assert_eq!(bal, u128::MAX);
    Ok(())
}

#[test]
fn test_malformed_transfer_parcel() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![50],
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Opcode 40 = malformed transfer parcel — should be handled safely
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
                inputs: vec![40],
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
    runtime.index_block(&block5, 5)?;
    println!("malformed_transfer_parcel test passed — no crash");
    Ok(())
}

#[test]
fn test_malformed_transfer_parcel_extcall() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![50],
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Opcode 41 = malformed transfer parcel via extcall
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
                inputs: vec![41],
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
    runtime.index_block(&block5, 5)?;
    println!("malformed_transfer_parcel_extcall test passed — no crash");
    Ok(())
}
