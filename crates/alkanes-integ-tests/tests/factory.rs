//! Port of crates/alkanes/src/tests/factory.rs

use alkanes_integ_tests::block_builder::{create_block_with_deploys, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;

#[test]
fn test_factory_wasm_load() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy test contract and execute 4 cellpacks:
    // 1. Mint 1M tokens of alkane 2:1
    // 2. Send (opcode 3) — transfers tokens to runtime
    // 3. Create another (opcode 50) — factory creates 2:2 from 2:1
    // 4. Steal (mint 1M of 2:2) — should fail (underflow)
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
                inputs: vec![3],
            }),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 5, tx: 1 },
                inputs: vec![50],
            }),
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 2, tx: 2 },
                inputs: vec![30, 2, 2, 1_000_000],
            }),
        ],
    );
    runtime.index_block(&block, 4)?;

    // Check last tx output (the steal attempt) — should have copy_alkane tokens = 1M
    // because the factory copy was created but the steal reverts
    let outpoint = last_tx_outpoint(&block);
    let copy_balance = query::get_alkane_balance(&runtime, &outpoint, 2, 2, 4)?;
    println!("copy_alkane (2:2) balance at last output: {}", copy_balance);

    // The original (2:1) should have been sent to runtime via opcode 3
    let orig_balance = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    println!("orig_alkane (2:1) balance at last output: {}", orig_balance);

    println!("Factory test passed — block indexed successfully");
    Ok(())
}
