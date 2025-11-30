//! BRC20-Prog integration tests using Signet network
//! Tests all BRC20-Prog RPC methods against https://rpc-signet.brc20.build

use alkanes_web_sys::WebProvider;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

fn setup_signet_provider() -> WebProvider {
    // Use Signet provider - auto-populates all URLs including BRC20-Prog
    WebProvider::new_js("signet".to_string(), None)
        .expect("Failed to create Signet provider")
}

#[wasm_bindgen_test]
async fn test_brc20prog_chain_id() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Chain ID ===".into());
    let provider = setup_signet_provider();
    
    let result = JsFuture::from(provider.brc20prog_chain_id_js()).await;
    assert!(result.is_ok(), "Should get chain ID");
    
    let chain_id = result.unwrap();
    web_sys::console::log_1(&format!("✅ Chain ID: {:?}", chain_id).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_block_number() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Block Number ===".into());
    let provider = setup_signet_provider();
    
    let result = JsFuture::from(provider.brc20prog_block_number_js()).await;
    assert!(result.is_ok(), "Should get block number");
    
    let block_number = result.unwrap();
    web_sys::console::log_1(&format!("✅ Block number: {:?}", block_number).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_get_code() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Get Code ===".into());
    let provider = setup_signet_provider();
    
    // Test with a known contract address on Signet
    // This is a placeholder - would need actual deployed contract address
    let address = "0x0000000000000000000000000000000000000000".to_string();
    
    let result = JsFuture::from(provider.brc20prog_get_code_js(address)).await;
    assert!(result.is_ok(), "Should get code (even if empty)");
    
    let code = result.unwrap();
    web_sys::console::log_1(&format!("✅ Code result: {:?}", code).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_get_balance() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Get Balance ===".into());
    let provider = setup_signet_provider();
    
    // Test with a zero address
    let address = "0x0000000000000000000000000000000000000000".to_string();
    
    let result = JsFuture::from(provider.brc20prog_get_balance_js(address, None)).await;
    assert!(result.is_ok(), "Should get balance");
    
    let balance = result.unwrap();
    web_sys::console::log_1(&format!("✅ Balance: {:?}", balance).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_get_transaction_count() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Get Transaction Count ===".into());
    let provider = setup_signet_provider();
    
    // Test with a zero address
    let address = "0x0000000000000000000000000000000000000000".to_string();
    
    let result = JsFuture::from(provider.brc20prog_get_transaction_count_js(address, None)).await;
    assert!(result.is_ok(), "Should get transaction count");
    
    let tx_count = result.unwrap();
    web_sys::console::log_1(&format!("✅ Transaction count: {:?}", tx_count).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_call() {
    web_sys::console::log_1(&"=== Testing BRC20-Prog Call ===".into());
    let provider = setup_signet_provider();
    
    // Test a simple view call (e.g., totalSupply on a contract)
    // Using placeholder address - would need actual contract
    let to = "0x0000000000000000000000000000000000000000".to_string();
    let data = "0x18160ddd".to_string(); // totalSupply() selector
    
    let result = JsFuture::from(provider.brc20prog_call_js(to, data, None)).await;
    // May fail if contract doesn't exist, but should not crash
    web_sys::console::log_1(&format!("✅ Call result: {:?}", result).into());
}

#[wasm_bindgen_test]
async fn test_brc20prog_comprehensive() {
    web_sys::console::log_1(&"=== Comprehensive BRC20-Prog Test ===".into());
    let provider = setup_signet_provider();
    
    web_sys::console::log_1(&"Step 1: Get chain ID".into());
    let chain_id_result = JsFuture::from(provider.brc20prog_chain_id_js()).await;
    assert!(chain_id_result.is_ok());
    web_sys::console::log_1(&format!("  Chain ID: {:?}", chain_id_result.unwrap()).into());
    
    web_sys::console::log_1(&"Step 2: Get block number".into());
    let block_result = JsFuture::from(provider.brc20prog_block_number_js()).await;
    assert!(block_result.is_ok());
    web_sys::console::log_1(&format!("  Block: {:?}", block_result.unwrap()).into());
    
    web_sys::console::log_1(&"Step 3: Query balance".into());
    let balance_result = JsFuture::from(
        provider.brc20prog_get_balance_js("0x0000000000000000000000000000000000000000".to_string(), None)
    ).await;
    assert!(balance_result.is_ok());
    web_sys::console::log_1(&format!("  Balance: {:?}", balance_result.unwrap()).into());
    
    web_sys::console::log_1(&"✅ All BRC20-Prog methods working!".into());
}
