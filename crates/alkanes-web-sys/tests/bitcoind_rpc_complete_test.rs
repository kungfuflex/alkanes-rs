//! Comprehensive Bitcoin RPC tests

use alkanes_web_sys::WebProvider;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

fn setup_provider() -> WebProvider {
    WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Failed to create regtest provider")
}

#[wasm_bindgen_test]
async fn test_bitcoind_get_block_count() {
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.bitcoind_get_block_count_js()).await;
    
    match result {
        Ok(count_js) => {
            let count = count_js.as_f64().unwrap() as u64;
            web_sys::console::log_1(&format!("✅ Block count: {}", count).into());
            assert!(count > 0);
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed: {:?}", e).into());
            panic!("Failed");
        }
    }
}

#[wasm_bindgen_test]
async fn test_bitcoind_get_blockchain_info() {
    let provider = setup_provider();
    let result = JsFuture::from(provider.bitcoind_get_blockchain_info_js()).await;
    assert!(result.is_ok());
    web_sys::console::log_1(&"✅ Blockchain info OK".into());
}

#[wasm_bindgen_test]
async fn test_bitcoind_get_network_info() {
    let provider = setup_provider();
    let result = JsFuture::from(provider.bitcoind_get_network_info_js()).await;
    assert!(result.is_ok());
    web_sys::console::log_1(&"✅ Network info OK".into());
}

#[wasm_bindgen_test]
async fn test_bitcoind_workflow() {
    web_sys::console::log_1(&"=== Bitcoin RPC Workflow Test ===".into());
    let provider = setup_provider();
    
    // Get block count
    let count = JsFuture::from(provider.bitcoind_get_block_count_js()).await;
    assert!(count.is_ok());
    web_sys::console::log_1(&"✅ Step 1: Block count OK".into());
    
    // Get block hash
    let hash_result = JsFuture::from(provider.bitcoind_get_block_hash_js(1.0)).await;
    assert!(hash_result.is_ok());
    let hash = hash_result.unwrap().as_string().unwrap();
    web_sys::console::log_1(&format!("✅ Step 2: Block hash: {}", hash).into());
    
    // Get block
    let block = JsFuture::from(provider.bitcoind_get_block_js(hash.clone(), false)).await;
    assert!(block.is_ok());
    web_sys::console::log_1(&"✅ Step 3: Block OK".into());
    
    // Get block header  
    let header = JsFuture::from(provider.bitcoind_get_block_header_js(hash.clone())).await;
    assert!(header.is_ok());
    web_sys::console::log_1(&"✅ Step 4: Block header OK".into());
    
    // Get block stats
    let stats = JsFuture::from(provider.bitcoind_get_block_stats_js(hash)).await;
    assert!(stats.is_ok());
    web_sys::console::log_1(&"✅ Step 5: Block stats OK".into());
    
    // Get mempool info
    let mempool = JsFuture::from(provider.bitcoind_get_mempool_info_js()).await;
    assert!(mempool.is_ok());
    web_sys::console::log_1(&"✅ Step 6: Mempool info OK".into());
    
    web_sys::console::log_1(&"✅ All tests passed!".into());
}
