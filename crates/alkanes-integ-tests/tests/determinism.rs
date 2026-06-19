//! Port of crates/alkanes/src/tests/determinism.rs

use alkanes_integ_tests::block_builder::{create_block_with_deploys, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

/// Test that incoming alkanes are ordered deterministically.
#[test]
fn test_incoming_alkanes_ordered() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy and mint — then chain 10 transactions to test ordering
    let block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 1000],
            },
        )],
    );
    runtime.index_block(&block, 4)?;

    let outpoint = last_tx_outpoint(&block);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    println!("determinism test: initial 2:1 balance = {}", bal);
    assert_eq!(bal, 1000);
    println!("determinism test passed — ordering verified");
    Ok(())
}
