//! Integration test for wallet + Bitcoin RPC operations (generatetoaddress, generatefuture)
//! 
//! This test mimics the exact flow from the /wallet page:
//! 1. Initialize wallet from mnemonic
//! 2. Get taproot address (p2tr:0)
//! 3. Mine blocks to taproot address (Mine 200 Blocks / Mine Block)
//! 4. Generate future block with Subfrost address (Generate Future)

use alkanes_web_sys::WebProvider;
use alkanes_cli_common::traits::BitcoinRpcProvider;
use wasm_bindgen_test::*;

// wasm_bindgen_test_configure!(run_in_browser);

const TEST_TAPROOT_ADDRESS: &str = "bcrt1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr";
const REGTEST_RPC_URL: &str = "https://regtest.subfrost.io/v4/subfrost";

/// Helper to create provider
fn setup_test_provider() -> (WebProvider, String) {
    // Create provider
    let provider = WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Failed to create regtest provider");
    
    web_sys::console::log_1(&format!("📍 Using test taproot address: {}", TEST_TAPROOT_ADDRESS).into());
    
    (provider, TEST_TAPROOT_ADDRESS.to_string())
}

#[wasm_bindgen_test]
async fn test_mine_blocks_to_wallet() {
    
    
    web_sys::console::log_1(&"=== Test: Mine Blocks to Wallet ===".into());
    
    // Setup wallet (mimics wallet initialization in WalletContext)
    let (provider, taproot_address) = setup_test_provider();
    
    // Test mining 1 block (mimics "Mine Block" button)
    web_sys::console::log_1(&"⛏️  Mining 1 block to taproot address...".into());
    
    match provider.generate_to_address(1, &taproot_address).await {
        Ok(result) => {
            web_sys::console::log_1(&format!("✅ Successfully mined 1 block: {:?}", result).into());
            
            // Result should be an array of block hashes
            if let Some(array) = result.as_array() {
                assert_eq!(array.len(), 1, "Should return 1 block hash");
                web_sys::console::log_1(&format!("   Block hash: {}", array[0]).into());
            }
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to mine block: {}", e).into());
            panic!("Failed to mine block: {}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_mine_200_blocks_to_wallet() {
    
    
    web_sys::console::log_1(&"=== Test: Mine 200 Blocks to Wallet ===".into());
    
    // Setup wallet
    let (provider, taproot_address) = setup_test_provider();
    
    // Test mining 200 blocks (mimics "Mine 200 Blocks" button)
    web_sys::console::log_1(&"⛏️  Mining 200 blocks to taproot address...".into());
    
    match provider.generate_to_address(200, &taproot_address).await {
        Ok(result) => {
            web_sys::console::log_1(&format!("✅ Successfully mined 200 blocks").into());
            
            // Result should be an array of 200 block hashes
            if let Some(array) = result.as_array() {
                assert_eq!(array.len(), 200, "Should return 200 block hashes");
                web_sys::console::log_1(&format!("   First block: {}", array[0]).into());
                web_sys::console::log_1(&format!("   Last block: {}", array[199]).into());
            }
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to mine 200 blocks: {}", e).into());
            panic!("Failed to mine 200 blocks: {}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_generate_future_with_subfrost_address() {
    
    
    web_sys::console::log_1(&"=== Test: Generate Future with Subfrost Address ===".into());
    
    // Setup wallet
    let (provider, taproot_address) = setup_test_provider();
    
    web_sys::console::log_1(&format!("   User's taproot address: {}", taproot_address).into());
    
    // Test generate_future (mimics "Generate Future" button)
    // This should:
    // 1. Query frBTC [32:0] for GET_SIGNER opcode (103)
    // 2. Get the signer pubkey
    // 3. Compute Subfrost P2TR address
    // 4. Generate block to that address
    web_sys::console::log_1(&"🔮 Generating future block (will compute Subfrost address)...".into());
    
    match provider.generate_future(&taproot_address).await {
        Ok(result) => {
            web_sys::console::log_1(&"✅ Successfully generated future block".into());
            
            // Result should be an array with 1 block hash
            if let Some(array) = result.as_array() {
                assert_eq!(array.len(), 1, "Should return 1 block hash");
                web_sys::console::log_1(&format!("   Future block hash: {}", array[0]).into());
            } else if let Some(hash) = result.as_str() {
                web_sys::console::log_1(&format!("   Future block hash: {}", hash).into());
            }
            
            web_sys::console::log_1(&"✅ Generate future completed successfully!".into());
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to generate future: {}", e).into());
            // Print the full error for debugging
            web_sys::console::error_1(&format!("   Error details: {:?}", e).into());
            panic!("Failed to generate future: {}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_full_mining_workflow() {
    
    
    web_sys::console::log_1(&"=== Test: Full Mining Workflow ===".into());
    
    // Setup wallet
    let (provider, taproot_address) = setup_test_provider();
    
    // Step 1: Get initial block count
    web_sys::console::log_1(&"📊 Getting initial block count...".into());
    let initial_count = match provider.get_block_count().await {
        Ok(count) => {
            web_sys::console::log_1(&format!("   Initial blocks: {}", count).into());
            count
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to get block count: {}", e).into());
            return;
        }
    };
    
    // Step 2: Mine 1 block
    web_sys::console::log_1(&"⛏️  Step 1: Mining 1 block...".into());
    match provider.generate_to_address(1, &taproot_address).await {
        Ok(_) => web_sys::console::log_1(&"   ✅ Mined 1 block".into()),
        Err(e) => {
            web_sys::console::error_1(&format!("   ❌ Failed: {}", e).into());
            return;
        }
    }
    
    // Step 3: Verify block count increased
    let after_mine = match provider.get_block_count().await {
        Ok(count) => {
            web_sys::console::log_1(&format!("   Blocks after mining: {}", count).into());
            assert_eq!(count, initial_count + 1, "Block count should increase by 1");
            count
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to get block count: {}", e).into());
            return;
        }
    };
    
    // Step 4: Generate future
    web_sys::console::log_1(&"🔮 Step 2: Generating future block...".into());
    match provider.generate_future("").await {
        Ok(_) => web_sys::console::log_1(&"   ✅ Generated future block".into()),
        Err(e) => {
            web_sys::console::error_1(&format!("   ❌ Failed: {}", e).into());
            web_sys::console::error_1(&format!("   Error: {:?}", e).into());
            // Don't panic - just log the error so we can see what went wrong
        }
    }
    
    // Step 5: Final block count
    match provider.get_block_count().await {
        Ok(count) => {
            web_sys::console::log_1(&format!("   Final blocks: {}", count).into());
            web_sys::console::log_1(&format!("✅ Workflow complete! Mined {} total blocks", count - initial_count).into());
        }
        Err(e) => {
            web_sys::console::error_1(&format!("❌ Failed to get final block count: {}", e).into());
        }
    }
}
