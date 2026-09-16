//! Smoke test: index empty blocks through all three indexers.

use alkanes_integ_tests::runtime::TestRuntime;
use anyhow::Result;

#[test]
fn test_index_empty_blocks() -> Result<()> {
    let _ = env_logger::try_init();

    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 10)?;

    assert_eq!(runtime.height(), 9);
    println!("Successfully indexed 10 empty blocks through all 3 indexers");
    Ok(())
}
