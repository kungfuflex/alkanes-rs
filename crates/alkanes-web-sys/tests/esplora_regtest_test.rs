//! Esplora integration tests using Regtest network

use alkanes_web_sys::WebProvider;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

fn setup_provider() -> WebProvider {
    // Use subfrost-regtest provider - auto-populates all URLs
    WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Failed to create regtest provider")
}

#[wasm_bindgen_test]
async fn test_esplora_get_blocks_tip_height() {
    web_sys::console::log_1(&"=== Testing Esplora Get Blocks Tip Height ===".into());
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.esplora_get_blocks_tip_height_js()).await;
    assert!(result.is_ok(), "Should get tip height");
    
    let height = result.unwrap();
    web_sys::console::log_1(&format!("✅ Tip height: {:?}", height).into());
}

#[wasm_bindgen_test]
async fn test_esplora_get_blocks_tip_hash() {
    web_sys::console::log_1(&"=== Testing Esplora Get Blocks Tip Hash ===".into());
    let provider = setup_provider();
    
    let result = JsFuture::from(provider.esplora_get_blocks_tip_hash_js()).await;
    assert!(result.is_ok(), "Should get tip hash");
    
    let hash = result.unwrap();
    web_sys::console::log_1(&format!("✅ Tip hash: {:?}", hash).into());
}

#[wasm_bindgen_test]
async fn test_esplora_get_address_info() {
    web_sys::console::log_1(&"=== Testing Esplora Get Address Info ===".into());
    let provider = setup_provider();
    
    // Use a test address (regtest miner address or similar)
    let address = "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string();
    
    let result = JsFuture::from(provider.esplora_get_address_info_js(address)).await;
    // May not have data but should not crash
    web_sys::console::log_1(&format!("✅ Address info result: {:?}", result).into());
}

#[wasm_bindgen_test]
async fn test_esplora_get_address_utxo() {
    web_sys::console::log_1(&"=== Testing Esplora Get Address UTXOs ===".into());
    let provider = setup_provider();
    
    let address = "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string();
    
    let result = JsFuture::from(provider.esplora_get_address_utxo_js(address)).await;
    assert!(result.is_ok(), "Should get UTXOs (even if empty)");
    
    let utxos = result.unwrap();
    web_sys::console::log_1(&format!("✅ UTXOs result: {:?}", utxos).into());
}

#[wasm_bindgen_test]
async fn test_esplora_get_address_txs() {
    web_sys::console::log_1(&"=== Testing Esplora Get Address Transactions ===".into());
    let provider = setup_provider();
    
    let address = "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string();
    
    let result = JsFuture::from(provider.esplora_get_address_txs_js(address)).await;
    assert!(result.is_ok(), "Should get transactions");
    
    let txs = result.unwrap();
    web_sys::console::log_1(&format!("✅ Transactions result: {:?}", txs).into());
}

#[wasm_bindgen_test]
async fn test_esplora_comprehensive() {
    web_sys::console::log_1(&"=== Comprehensive Esplora Test ===".into());
    let provider = setup_provider();
    
    web_sys::console::log_1(&"Step 1: Get tip height".into());
    let height_result = JsFuture::from(provider.esplora_get_blocks_tip_height_js()).await;
    assert!(height_result.is_ok());
    web_sys::console::log_1(&format!("  Height: {:?}", height_result.unwrap()).into());
    
    web_sys::console::log_1(&"Step 2: Get tip hash".into());
    let hash_result = JsFuture::from(provider.esplora_get_blocks_tip_hash_js()).await;
    assert!(hash_result.is_ok());
    web_sys::console::log_1(&format!("  Hash: {:?}", hash_result.unwrap()).into());
    
    web_sys::console::log_1(&"Step 3: Query address UTXOs".into());
    let utxo_result = JsFuture::from(
        provider.esplora_get_address_utxo_js("bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string())
    ).await;
    assert!(utxo_result.is_ok());
    web_sys::console::log_1(&format!("  UTXOs: {:?}", utxo_result.unwrap()).into());
    
    web_sys::console::log_1(&"✅ All Esplora methods working!".into());
}
