//! Port of crates/alkanes/src/tests/vec_input_test.rs

use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

/// Test vector input processing (opcodes 11, 12, 13).
#[test]
fn test_vec_inputs() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy with opcode 11 (process_numbers)
    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::TEST_CONTRACT,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![11, 3, 100, 200, 300],
                },
            ),
            // Opcode 12 (process_strings)
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![12, 2, 5, 72, 101, 108, 108, 111, 5, 87, 111, 114, 108, 100],
            }),
            // Opcode 13 (process_nested)
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![13, 2, 3, 1, 2, 3, 2, 4, 5],
            }),
        ],
    );
    runtime.index_block(&block, 4)?;
    println!("vec_inputs test passed — all vector operations indexed");
    Ok(())
}
