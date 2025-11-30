//! Metashrew integration tests using Regtest network

use alkanes_web_sys::WebProvider;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

fn setup_provider() -> WebProvider {
    // Use subfrost-regtest provider - auto-populates all URLs
    WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Failed to create regtest provider")
}

#[wasm_bindgen_test]
async fn test_metashrew_height() {
    web_sys::console::log_1(&"=== Testing Metashrew Height ===".into());
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.metashrew_height_js()).await;
    assert!(result.is_ok(), "Should get metashrew height");
    
    let height = result.unwrap();
    web_sys::console::log_1(&format!("✅ Metashrew height: {:?}", height).into());
}

#[wasm_bindgen_test]
async fn test_metashrew_get_block_hash() {
    web_sys::console::log_1(&"=== Testing Metashrew Get Block Hash ===".into());
    let provider = setup_provider();
    
    // Get block hash at height 1 (should exist on regtest)
    let result = JsFuture::from(provider.metashrew_get_block_hash_js(1.0)).await;
    assert!(result.is_ok(), "Should get block hash");
    
    let hash = result.unwrap();
    web_sys::console::log_1(&format!("✅ Block hash at height 1: {:?}", hash).into());
}

#[wasm_bindgen_test]
async fn test_metashrew_state_root() {
    web_sys::console::log_1(&"=== Testing Metashrew State Root ===".into());
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.metashrew_state_root_js(None)).await;
    assert!(result.is_ok(), "Should get state root");
    
    let state_root = result.unwrap();
    web_sys::console::log_1(&format!("✅ State root: {:?}", state_root).into());
}

#[wasm_bindgen_test]
async fn test_metashrew_state_root_at_height() {
    web_sys::console::log_1(&"=== Testing Metashrew State Root at Height ===".into());
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.metashrew_state_root_js(Some(1.0))).await;
    assert!(result.is_ok(), "Should get state root at height");
    
    let state_root = result.unwrap();
    web_sys::console::log_1(&format!("✅ State root at height 1: {:?}", state_root).into());
}

#[wasm_bindgen_test]
async fn test_metashrew_comprehensive() {
    web_sys::console::log_1(&"=== Comprehensive Metashrew Test ===".into());
    let provider = setup_provider();
    
    web_sys::console::log_1(&"Step 1: Get current height".into());
    let height_result = JsFuture::from(provider.metashrew_height_js()).await;
    assert!(height_result.is_ok());
    let height = height_result.unwrap();
    web_sys::console::log_1(&format!("  Height: {:?}", height).into());
    
    web_sys::console::log_1(&"Step 2: Get block hash at height 1".into());
    let hash_result = JsFuture::from(provider.metashrew_get_block_hash_js(1.0)).await;
    assert!(hash_result.is_ok());
    let hash = hash_result.unwrap();
    web_sys::console::log_1(&format!("  Block hash: {:?}", hash).into());
    
    web_sys::console::log_1(&"Step 3: Get current state root".into());
    let state_result = JsFuture::from(provider.metashrew_state_root_js(None)).await;
    assert!(state_result.is_ok());
    let state = state_result.unwrap();
    web_sys::console::log_1(&format!("  State root: {:?}", state).into());
    
    web_sys::console::log_1(&"✅ All Metashrew methods working!".into());
}
