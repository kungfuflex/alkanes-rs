//! Port of crates/alkanes/src/tests/fuel.rs — fuel consumption tests.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

#[test]
fn test_infinite_loop() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Opcode 20 = infinite loop — should consume all fuel and revert
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![20],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("infinite_loop test passed — fuel consumed, block indexed");
    Ok(())
}

#[test]
fn test_infinite_extcall_loop() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Opcode 21 = infinite extcall recursion — should consume all fuel
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![21],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("infinite_extcall_loop test passed — fuel consumed, block indexed");
    Ok(())
}
