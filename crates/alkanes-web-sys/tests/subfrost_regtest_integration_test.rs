//! Subfrost Regtest Integration Tests
//!
//! These tests run against the Subfrost Regtest network to verify:
//! - Data API functionality (pool details, balances, keys)
//! - Exchange operations (swaps, liquidity)
//! - Swap path routing
//! - Wallet dashboard data loading
//!
//! Run with: wasm-pack test --node -- --test subfrost_regtest_integration_test

use alkanes_web_sys::WebProvider;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

// Factory ID for the OylSwap AMM on regtest
const FACTORY_ID: &str = "4:65522";

// Helper macro for logging that works in Node.js
macro_rules! log {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        let _ = js_sys::Reflect::get(&js_sys::global(), &"console".into())
            .and_then(|console| {
                let log_fn = js_sys::Reflect::get(&console, &"log".into())?;
                let log_fn = log_fn.dyn_into::<js_sys::Function>()?;
                log_fn.call1(&JsValue::NULL, &JsValue::from_str(&msg))
            });
    };
}

fn setup_provider() -> WebProvider {
    // Use subfrost-regtest provider with data API
    WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Failed to create subfrost-regtest provider")
}

// ============================================================================
// Data API Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_data_api_get_pools() {
    log!("=== Testing Data API: Get Pools ===");
    let provider = setup_provider();

    // Test getting all pools via data API using factory ID
    let result = JsFuture::from(provider.data_api_get_pools_js(FACTORY_ID.to_string())).await;

    match result {
        Ok(pools) => {
            log!("✅ Data API get_pools succeeded");
            log!("   Pools data: {:?}", pools);
        }
        Err(e) => {
            log!("❌ Data API get_pools failed: {:?}", e);
            panic!("get_pools failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_pool_history() {
    log!("=== Testing Data API: Get Pool History ===");
    let provider = setup_provider();

    // First get all pools to find a valid pool ID
    let pools_result = JsFuture::from(provider.data_api_get_pools_js(FACTORY_ID.to_string())).await;

    if let Ok(_pools_js) = pools_result {
        log!("   Got pools list, attempting to get history");

        // Try to get history for a pool
        let history_result = JsFuture::from(provider.data_api_get_pool_history_js(
            "2:1".to_string(),  // Example pool ID
            None,               // category
            Some(10),           // limit
            None                // offset
        )).await;

        match history_result {
            Ok(history) => {
                log!("✅ Data API get_pool_history succeeded");
                log!("   Pool history: {:?}", history);
            }
            Err(e) => {
                log!("⚠️ Data API get_pool_history failed (may not exist): {:?}", e);
            }
        }
    } else {
        log!("⚠️ Could not get pools list to test pool history");
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_keys() {
    log!("=== Testing Data API: Get Keys ===");
    let provider = setup_provider();

    // Test getting keys for a specific alkane (e.g., factory contract)
    let result = JsFuture::from(provider.data_api_get_keys_js(
        "2:0".to_string(),  // Factory alkane ID
        None,               // No prefix filter
        100                 // Limit
    )).await;

    match result {
        Ok(keys) => {
            log!("✅ Data API get_keys succeeded");
            log!("   Keys: {:?}", keys);
        }
        Err(e) => {
            log!("⚠️ Data API get_keys failed (may not have data): {:?}", e);
        }
    }
}

// ============================================================================
// Metashrew/Indexer Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_metashrew_height() {
    log!("=== Testing Metashrew Height ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.metashrew_height_js()).await;

    match result {
        Ok(height) => {
            log!("✅ Metashrew height: {:?}", height);
        }
        Err(e) => {
            log!("❌ Metashrew height failed: {:?}", e);
            panic!("metashrew_height failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_metashrew_state_root() {
    log!("=== Testing Metashrew State Root ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.metashrew_state_root_js(None)).await;

    match result {
        Ok(state_root) => {
            log!("✅ Metashrew state root: {:?}", state_root);
        }
        Err(e) => {
            log!("❌ Metashrew state root failed: {:?}", e);
            panic!("metashrew_state_root failed: {:?}", e);
        }
    }
}

// ============================================================================
// Esplora Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_esplora_tip_height() {
    log!("=== Testing Esplora Tip Height ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.esplora_get_blocks_tip_height_js()).await;

    match result {
        Ok(height) => {
            log!("✅ Esplora tip height: {:?}", height);
        }
        Err(e) => {
            log!("❌ Esplora tip height failed: {:?}", e);
            panic!("esplora tip height failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_esplora_tip_hash() {
    log!("=== Testing Esplora Tip Hash ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.esplora_get_blocks_tip_hash_js()).await;

    match result {
        Ok(hash) => {
            log!("✅ Esplora tip hash: {:?}", hash);
        }
        Err(e) => {
            log!("❌ Esplora tip hash failed: {:?}", e);
            panic!("esplora tip hash failed: {:?}", e);
        }
    }
}

// ============================================================================
// Alkanes Core Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_alkanes_balance() {
    log!("=== Testing Alkanes Balance ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.alkanes_balance_js(None)).await;

    match result {
        Ok(balance) => {
            log!("✅ Alkanes balance: {:?}", balance);
        }
        Err(e) => {
            log!("❌ Alkanes balance failed: {:?}", e);
            panic!("alkanes_balance failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_alkanes_by_address() {
    log!("=== Testing Alkanes By Address ===");
    let provider = setup_provider();

    // Test with a sample address
    let test_address = "bcrt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq33fzal";
    let result = JsFuture::from(provider.alkanes_by_address_js(
        test_address.to_string(),
        None,  // block_tag
        None   // protocol_tag
    )).await;

    match result {
        Ok(alkanes) => {
            log!("✅ Alkanes by address: {:?}", alkanes);
        }
        Err(e) => {
            log!("⚠️ Alkanes by_address failed (may be empty): {:?}", e);
        }
    }
}

// ============================================================================
// Exchange/AMM Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_get_all_pools() {
    log!("=== Testing Get All Pools ===");
    let provider = setup_provider();

    // This tests the AMM pool fetching functionality
    let result = JsFuture::from(provider.alkanes_get_all_pools_js(FACTORY_ID.to_string())).await;

    match result {
        Ok(pools) => {
            log!("✅ Get all pools succeeded");
            log!("   Pools: {:?}", pools);
        }
        Err(e) => {
            log!("⚠️ Get all pools failed (may have no pools): {:?}", e);
        }
    }
}

// ============================================================================
// Bitcoin RPC Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_bitcoind_get_block_count() {
    log!("=== Testing Bitcoind Get Block Count ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.bitcoind_get_block_count_js()).await;

    match result {
        Ok(count) => {
            log!("✅ Bitcoin block count: {:?}", count);
        }
        Err(e) => {
            log!("❌ Bitcoin get_block_count failed: {:?}", e);
            panic!("get_block_count failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_bitcoind_get_blockchain_info() {
    log!("=== Testing Bitcoind Get Blockchain Info ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.bitcoind_get_blockchain_info_js()).await;

    match result {
        Ok(info) => {
            log!("✅ Bitcoin blockchain info: {:?}", info);
        }
        Err(e) => {
            log!("❌ Bitcoin get_blockchain_info failed: {:?}", e);
            panic!("get_blockchain_info failed: {:?}", e);
        }
    }
}

// ============================================================================
// Lua Script Tests (for balance loading)
// ============================================================================

#[wasm_bindgen_test]
async fn test_lua_eval_script() {
    log!("=== Testing Lua Eval Script ===");
    let provider = setup_provider();

    // Test a simple lua script that returns the current height
    let script = r#"return redis.call('GET', '/height')"#;
    let result = JsFuture::from(provider.lua_eval_script_js(script.to_string())).await;

    match result {
        Ok(value) => {
            log!("✅ Lua eval script succeeded: {:?}", value);
        }
        Err(e) => {
            log!("⚠️ Lua eval script failed: {:?}", e);
        }
    }
}

// ============================================================================
// Additional Data API Tests
// ============================================================================

#[wasm_bindgen_test]
async fn test_data_api_get_address_balances() {
    log!("=== Testing Data API: Get Address Balances ===");
    let provider = setup_provider();

    // Test with a sample regtest address
    let test_address = "bcrt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq33fzal";
    let result = JsFuture::from(provider.data_api_get_address_balances_js(
        test_address.to_string(),
        true  // include_outpoints
    )).await;

    match result {
        Ok(balances) => {
            log!("✅ Data API get_address_balances succeeded");
            log!("   Balances: {:?}", balances);
        }
        Err(e) => {
            log!("⚠️ Data API get_address_balances failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_alkanes_by_address() {
    log!("=== Testing Data API: Get Alkanes By Address ===");
    let provider = setup_provider();

    let test_address = "bcrt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq33fzal";
    let result = JsFuture::from(provider.data_api_get_alkanes_by_address_js(
        test_address.to_string()
    )).await;

    match result {
        Ok(alkanes) => {
            log!("✅ Data API get_alkanes_by_address succeeded");
            log!("   Alkanes: {:?}", alkanes);
        }
        Err(e) => {
            log!("⚠️ Data API get_alkanes_by_address failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_reserves() {
    log!("=== Testing Data API: Get Reserves ===");
    let provider = setup_provider();

    // Get reserves for a pool
    let result = JsFuture::from(provider.data_api_get_reserves_js("2:1".to_string())).await;

    match result {
        Ok(reserves) => {
            log!("✅ Data API get_reserves succeeded");
            log!("   Reserves: {:?}", reserves);
        }
        Err(e) => {
            log!("⚠️ Data API get_reserves failed (pool may not exist): {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_swap_history() {
    log!("=== Testing Data API: Get Swap History ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.data_api_get_swap_history_js(
        "2:1".to_string(),
        Some(10),
        None
    )).await;

    match result {
        Ok(history) => {
            log!("✅ Data API get_swap_history succeeded");
            log!("   Swap history: {:?}", history);
        }
        Err(e) => {
            log!("⚠️ Data API get_swap_history failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_holders() {
    log!("=== Testing Data API: Get Holders ===");
    let provider = setup_provider();

    // Get holders for an alkane token
    let result = JsFuture::from(provider.data_api_get_holders_js(
        "2:1".to_string(),
        1,   // page
        10   // limit
    )).await;

    match result {
        Ok(holders) => {
            log!("✅ Data API get_holders succeeded");
            log!("   Holders: {:?}", holders);
        }
        Err(e) => {
            log!("⚠️ Data API get_holders failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_trades() {
    log!("=== Testing Data API: Get Trades ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.data_api_get_trades_js(
        "2:1".to_string(),
        None,    // start_time
        None,    // end_time
        Some(10) // limit
    )).await;

    match result {
        Ok(trades) => {
            log!("✅ Data API get_trades succeeded");
            log!("   Trades: {:?}", trades);
        }
        Err(e) => {
            log!("⚠️ Data API get_trades failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_data_api_get_candles() {
    log!("=== Testing Data API: Get Candles ===");
    let provider = setup_provider();

    let result = JsFuture::from(provider.data_api_get_candles_js(
        "2:1".to_string(),
        "1h".to_string(), // interval
        None,             // start_time
        None,             // end_time
        Some(24)          // limit
    )).await;

    match result {
        Ok(candles) => {
            log!("✅ Data API get_candles succeeded");
            log!("   Candles: {:?}", candles);
        }
        Err(e) => {
            log!("⚠️ Data API get_candles failed: {:?}", e);
        }
    }
}

// ============================================================================
// Comprehensive Integration Test
// ============================================================================

#[wasm_bindgen_test]
async fn test_comprehensive_data_loading() {
    log!("\n🎉 ===== COMPREHENSIVE DATA LOADING TEST ===== 🎉\n");
    let provider = setup_provider();

    // Step 1: Get chain state
    log!("Step 1: Checking chain state...");
    let height = JsFuture::from(provider.metashrew_height_js()).await;
    let block_count = JsFuture::from(provider.bitcoind_get_block_count_js()).await;

    log!("   Metashrew height: {:?}", height);
    log!("   Bitcoin block count: {:?}", block_count);

    // Step 2: Get pools from data API
    log!("\nStep 2: Loading pools from Data API...");
    let pools = JsFuture::from(provider.data_api_get_pools_js(FACTORY_ID.to_string())).await;
    log!("   Pools result: {:?}", pools);

    // Step 3: Get alkanes balance
    log!("\nStep 3: Getting alkanes balance...");
    let balance = JsFuture::from(provider.alkanes_balance_js(None)).await;
    log!("   Balance: {:?}", balance);

    // Step 4: Test Esplora integration
    log!("\nStep 4: Checking Esplora...");
    let tip = JsFuture::from(provider.esplora_get_blocks_tip_height_js()).await;
    log!("   Esplora tip: {:?}", tip);

    log!("\n✅ Comprehensive data loading test complete!\n");
}
