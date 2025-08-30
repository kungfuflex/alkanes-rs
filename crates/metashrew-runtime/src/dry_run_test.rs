// crates/metashrew-runtime/src/tests/dry_run_test.rs

use crate::{MetashrewRuntime, test_utils::MemStoreAdapter};

#[tokio::test]
async fn test_dry_run_captures_sets() {
    let _ = env_logger::builder().is_test(true).try_init();
    let wasm_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/dry_run_test.wasm"));
    let db = MemStoreAdapter::new();
    let mut runtime = MetashrewRuntime::new(wasm_bytes, db).unwrap();

    let (read_set, write_set) = runtime.dry_run(&vec![], 0).unwrap();

    assert_eq!(read_set.len(), 2, "Should capture two reads");
    assert_eq!(read_set[0], b"read_key_1".to_vec());
    assert_eq!(read_set[1], b"read_key_2".to_vec());

    // The __flush implementation in the runtime parses a KeyValueFlush message.
    // The WAT file simulates this by creating a byte array of concatenated keys and values.
    // The runtime's __flush function will parse this and extract the keys.
    assert_eq!(write_set.len(), 2, "Should capture two writes from the flush operation");
    assert_eq!(write_set[0], b"write_key_1".to_vec());
    assert_eq!(write_set[1], b"write_key_2".to_vec());
}