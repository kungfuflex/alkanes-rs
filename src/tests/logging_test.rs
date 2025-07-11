// Test for the new consolidated logging system

use crate::{alkane_log, logging::*};
use alkanes_support::id::AlkaneId;
use anyhow::Result;

#[test]
fn test_block_stats_tracking() -> Result<()> {
    // Initialize block stats
    init_block_stats();

    // Record some metrics
    record_transaction();
    record_transaction();
    record_outpoints(10);
    record_protostone_run();
    record_protostone_with_cellpack();
    record_fuel_consumed(50000);
    record_excess_fuel_unused(5000);

    // Record an alkane creation
    let creation = AlkaneCreation {
        alkane_id: AlkaneId { block: 2, tx: 1 },
        wasm_size_kb: 15.5,
        creation_method: CreationMethod::DirectInit,
    };
    record_alkane_creation(creation);

    // Test cache stats
    let cache_stats = CacheStats {
        hits: 100,
        misses: 20,
        current_size: 500,
        max_capacity: 1000,
        evictions: 5,
        memory_usage: 500,
    };
    update_cache_stats(cache_stats);

    // Verify stats were recorded
    let stats = get_block_stats();
    assert!(stats.is_some());
    let s = stats.unwrap();
    assert_eq!(s.transactions_processed, 2);
    assert_eq!(s.outpoints_indexed, 10);
    assert_eq!(s.protostones_run, 1);
    assert_eq!(s.protostones_with_cellpacks, 1);
    assert_eq!(s.total_fuel_consumed, 50000);
    assert_eq!(s.excess_fuel_unused, 5000);
    assert_eq!(s.new_alkanes.len(), 1);
    assert_eq!(s.cache_stats.hits, 100);

    Ok(())
}

#[test]
fn test_creation_method_determination() {
    let target1 = AlkaneId { block: 1, tx: 0 };
    let resolved1 = AlkaneId { block: 2, tx: 1 };
    let method1 = determine_creation_method(&target1, &resolved1);
    assert!(matches!(method1, CreationMethod::DirectInit));

    let target2 = AlkaneId {
        block: 3,
        tx: 12345,
    };
    let resolved2 = AlkaneId {
        block: 4,
        tx: 12345,
    };
    let method2 = determine_creation_method(&target2, &resolved2);
    assert!(matches!(method2, CreationMethod::PredictableAddress(12345)));

    let target3 = AlkaneId { block: 5, tx: 100 };
    let resolved3 = AlkaneId { block: 2, tx: 50 };
    let method3 = determine_creation_method(&target3, &resolved3);
    assert!(matches!(method3, CreationMethod::FactoryClone(_)));

    let target4 = AlkaneId { block: 6, tx: 200 };
    let resolved4 = AlkaneId { block: 2, tx: 75 };
    let method4 = determine_creation_method(&target4, &resolved4);
    assert!(matches!(
        method4,
        CreationMethod::FactoryClonePredictable(_)
    ));
}

#[test]
fn test_wasm_size_calculation() {
    let wasm_bytes_1kb = vec![0u8; 1024];
    assert_eq!(calculate_wasm_size_kb(&wasm_bytes_1kb), 1.0);

    let wasm_bytes_2_5kb = vec![0u8; 2560];
    assert_eq!(calculate_wasm_size_kb(&wasm_bytes_2_5kb), 2.5);

    let wasm_bytes_empty = vec![];
    assert_eq!(calculate_wasm_size_kb(&wasm_bytes_empty), 0.0);
}

#[test]
fn test_alkane_log_macro() {
    // Test that the alkane_log macro compiles correctly
    // The actual logging behavior depends on the "logs" feature flag
    alkane_log!("Test message: {}", "hello world");
    alkane_log!("Test with numbers: {} and {}", 42, 3.14);
}
