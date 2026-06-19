//! Port of crates/alkanes/src/tests/alkane.rs
//!
//! Tests: compression, extcall, transaction, benchmark

use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::gz::{compress, decompress};
use alkanes_support::id::AlkaneId;
use anyhow::Result;

#[test]
fn test_compression() -> Result<()> {
    let buffer = fixtures::TEST_CONTRACT.to_vec();
    let compressed = compress(buffer.clone())?;
    assert_eq!(decompress(compressed)?, buffer);
    Ok(())
}

#[test]
fn test_extcall() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::TEST_CONTRACT,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![1],
                },
            ),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            }),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![50, 1],
            }),
        ],
    );
    runtime.index_block(&block, 4)?;
    println!("extcall test passed — block indexed successfully");
    Ok(())
}

#[test]
fn test_transaction() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::TEST_CONTRACT,
                Cellpack {
                    target: AlkaneId {
                        block: 3,
                        tx: 10001,
                    },
                    inputs: vec![0, 0],
                },
            ),
            DeployPair::call_only(Cellpack {
                target: AlkaneId {
                    block: 4,
                    tx: 10001,
                },
                inputs: vec![50],
            }),
        ],
    );
    runtime.index_block(&block, 4)?;
    println!("transaction test passed — block indexed successfully");
    Ok(())
}

#[test]
fn test_benchmark() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let start = std::time::Instant::now();
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![78],
            },
        )],
    );
    runtime.index_block(&block, 4)?;
    println!("benchmark time: {:?}", start.elapsed());
    Ok(())
}
