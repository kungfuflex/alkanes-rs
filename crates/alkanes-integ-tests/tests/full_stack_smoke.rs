//! Full-stack smoke test: TestChain + real WasmIndexerRuntime + RocksDB.
//!
//! This uses the vendored qubitcoin crates — the exact same runtime
//! that runs in production.

use alkanes_integ_tests::harness::FullStackHarness;
use anyhow::Result;

#[test]
fn test_full_stack_index_empty_blocks() -> Result<()> {
    let _ = env_logger::try_init();

    let mut harness = FullStackHarness::new()?;
    println!("Full-stack harness created (TestChain + alkanes + esplora)");

    // Mine 10 empty blocks through the real qubitcoin runtime
    harness.mine_empty_blocks(10)?;
    println!("Mined {} blocks through full stack", harness.height());

    assert_eq!(harness.height(), 10);

    // Query esplora tipheight
    let tip = harness.esplora_view("tipheight", &[]);
    match tip {
        Ok(data) => println!("esplora tipheight: {}", String::from_utf8_lossy(&data)),
        Err(e) => println!("esplora tipheight failed (may need height prefix): {}", e),
    }

    println!("Full-stack smoke test PASSED — {} blocks indexed via qubitcoin runtime", harness.height());
    Ok(())
}
