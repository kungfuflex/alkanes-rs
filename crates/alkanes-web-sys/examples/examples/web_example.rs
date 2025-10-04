//! Example demonstrating deezel-web usage
//!
//! This example shows how to use the WebProvider in a web environment.
//! Note: This example is designed to run in a browser environment with WASM.

use deezel_web::prelude::*;
use deezel_web::{web_info, web_error, web_log};

// This is the main entry point for the WASM module
#[wasm_bindgen]
#[allow(clippy::main_recursion)]
#[cfg(not(test))]
pub fn main() {
    wasm_bindgen_futures::spawn_local(async_main());
}

async fn async_main() {
    // Initialize the web library
    deezel_web::init();
    
    // Create a web provider
    let provider = match WebProvider::new(
        "regtest".to_string(),
    ).await {
        Ok(provider) => provider,
        Err(e) => {
            web_error!("Failed to create WebProvider: {}", e);
            return;
        }
    };
    
    web_info!("WebProvider created successfully");
    
    // Initialize the provider
    if let Err(e) = provider.initialize().await {
        web_error!("Failed to initialize provider: {}", e);
        return;
    }
    
    web_info!("Provider initialized successfully");
    
    // Test basic functionality
    test_basic_functionality(&provider).await;
    
    // Test storage operations
    test_storage_operations(&provider).await;
    
    // Test wallet operations
    test_wallet_operations(&provider).await;
    
    // Test alkanes operations
    test_alkanes_operations(&provider).await;
    
    web_info!("All tests completed");
}

async fn test_basic_functionality(provider: &WebProvider) {
    web_info!("Testing basic functionality...");
    
    // Test provider name
    let name = provider.provider_name();
    web_info!("Provider name: {}", name);
    
    // Test time operations
    let now_secs = provider.now_secs();
    let now_millis = provider.now_millis();
    web_info!("Current time: {} seconds, {} milliseconds", now_secs, now_millis);
    
    // Test random bytes generation
    match provider.random_bytes(32) {
        Ok(bytes) => web_info!("Generated {} random bytes", bytes.len()),
        Err(e) => web_error!("Failed to generate random bytes: {}", e),
    }
    
    // Test SHA256 hashing
    let test_data = b"Hello, deezel-web!";
    match provider.sha256(test_data) {
        Ok(hash) => web_info!("SHA256 hash: {}", hex::encode(hash)),
        Err(e) => web_error!("Failed to compute SHA256: {}", e),
    }
}

async fn test_storage_operations(provider: &WebProvider) {
    web_info!("Testing storage operations...");
    
    let test_key = "test_key";
    let test_data = b"test data for storage";
    
    // Test write
    match provider.write(test_key, test_data).await {
        Ok(_) => web_info!("Successfully wrote data to storage"),
        Err(e) => {
            web_error!("Failed to write to storage: {}", e);
            return;
        }
    }
    
    // Test exists
    match provider.exists(test_key).await {
        Ok(exists) => web_info!("Key exists: {}", exists),
        Err(e) => web_error!("Failed to check if key exists: {}", e),
    }
    
    // Test read
    match provider.read(test_key).await {
        Ok(data) => {
            if data == test_data {
                web_info!("Successfully read data from storage");
            } else {
                web_error!("Read data doesn't match written data");
            }
        },
        Err(e) => web_error!("Failed to read from storage: {}", e),
    }
    
    // Test delete
    match provider.delete(test_key).await {
        Ok(_) => web_info!("Successfully deleted data from storage"),
        Err(e) => web_error!("Failed to delete from storage: {}", e),
    }
}

async fn test_wallet_operations(provider: &WebProvider) {
    web_info!("Testing wallet operations...");
    
    let config = provider.get_wallet_config();
    
    // Test wallet creation
    match provider.create_wallet(config.clone(), None, None).await {
        Ok(wallet_info) => {
            web_info!("Created wallet with address: {}", wallet_info.address);
            web_info!("Network: {:?}", wallet_info.network);
        },
        Err(e) => {
            web_error!("Failed to create wallet: {}", e);
            return;
        }
    }
    
    // Test get balance
    match WalletProvider::get_balance(provider).await {
        Ok(balance) => {
            web_info!("Wallet balance - Confirmed: {} sats", balance.confirmed);
        },
        Err(e) => web_error!("Failed to get balance: {}", e),
    }
    
    // Test get address
    match WalletProvider::get_address(provider).await {
        Ok(address) => web_info!("Wallet address: {}", address),
        Err(e) => web_error!("Failed to get address: {}", e),
    }
    
    // Test get UTXOs
    match provider.get_utxos(false, None).await {
        Ok(utxos) => web_info!("Found {} UTXOs", utxos.len()),
        Err(e) => web_error!("Failed to get UTXOs: {}", e),
    }
}

async fn test_alkanes_operations(provider: &WebProvider) {
    web_info!("Testing alkanes operations...");
    
    // Test get balance
    match AlkanesProvider::get_balance(provider, None).await {
        Ok(balances) => {
            web_info!("Found {} alkanes balances", balances.len());
            for balance in balances {
                web_info!("Token: {} ({}), Balance: {}", balance.name, balance.symbol, balance.balance);
            }
        },
        Err(e) => web_error!("Failed to get alkanes balance: {}", e),
    }
    
    // Test alkanes execution (mock)
    let execute_params = AlkanesExecuteParams {
        inputs: Some("mock_inputs".to_string()),
        to: "mock_to_address".to_string(),
        change: None,
        fee_rate: Some(10.0),
        envelope: None,
        protostones: "mock_protostones".to_string(),
        trace: true,
        mine: false,
        auto_confirm: false,
        rebar: false, // Test without rebar first
    };
    
    match provider.execute(execute_params).await {
        Ok(result) => {
            web_info!("Alkanes execution successful");
            web_info!("Reveal TXID: {}", result.reveal_txid);
            if let Some(traces) = result.traces {
                web_info!("Traces: {:?}", traces);
            }
        },
        Err(e) => web_error!("Failed to execute alkanes: {}", e),
    }
    
    // Test alkanes inspection
    let inspect_config = AlkanesInspectConfig {
        disasm: true,
        fuzz: false,
        fuzz_ranges: None,
        meta: true,
        codehash: true,
    };
    
    match provider.inspect("800000:1", inspect_config).await {
        Ok(result) => {
            web_info!("Alkanes inspection successful");
            web_info!("Bytecode length: {}", result.bytecode_length);
            if let Some(metadata) = result.metadata {
                web_info!("Contract name: {}", metadata.name);
            }
        },
        Err(e) => web_error!("Failed to inspect alkanes: {}", e),
    }
}

// Export functions for JavaScript interop
#[wasm_bindgen]
pub async fn test_web_provider() {
    // The main function is the entry point of the example.
}

#[wasm_bindgen]
pub fn get_provider_info() -> String {
    format!("deezel-web v{}", deezel_web::VERSION)
}
