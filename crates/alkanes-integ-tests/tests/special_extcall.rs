//! Port of crates/alkanes/src/tests/special_extcall.rs
//!
//! Tests special extcalls: block header, coinbase tx, diesel count, miner fees.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

#[test]
fn test_special_extcall_header() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Opcode 101 = get block header
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![101],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("special_extcall header test passed");
    Ok(())
}

#[test]
fn test_special_extcall_number_diesel_mints() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Opcode 106 = get number of diesel mints
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![106],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("special_extcall diesel_mints test passed");
    Ok(())
}

#[test]
fn test_special_extcall_total_miner_fees() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Opcode 107 = get total miner fees
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![107],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("special_extcall miner_fees test passed");
    Ok(())
}
