use alkanes_native::adapters::NativeRuntimeAdapter;
use alkanes_native::rpc::handle_request;
use alkanes_native::test_utils::{MemStorageAdapter, MockNodeAdapter};
use bitcoin::blockdata::constants::genesis_block;
use bitcoin::Network as BitcoinNetwork;
use metashrew_sync::{MetashrewSync, SyncConfig};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_e2e_sequence() {
    let storage_adapter = MemStorageAdapter::default();
    let mut blocks = HashMap::new();
    let genesis = genesis_block(BitcoinNetwork::Regtest);
    blocks.insert(0, genesis.clone());
    let node_adapter = MockNodeAdapter {
        blocks: Arc::new(Mutex::new(blocks)),
    };
    let runtime_adapter = NativeRuntimeAdapter;
    let config = SyncConfig {
        start_block: 0,
        exit_at: Some(0),
        ..Default::default()
    };
    let mut sync_engine = MetashrewSync::new(
        node_adapter,
        storage_adapter.clone(),
        runtime_adapter,
        config,
    );
    sync_engine.start().await.unwrap();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "metashrew_view",
        "params": ["sequence", []],
        "id": 1
    })
    .to_string();

    let result = handle_request(storage_adapter, &request).await.unwrap();
    let expected = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": "[]"
    })
    .to_string();
    assert_eq!(result, expected);
}