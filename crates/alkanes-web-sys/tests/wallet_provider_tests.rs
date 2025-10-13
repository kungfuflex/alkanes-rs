//! Tests for the browser wallet provider system
//!
//! These tests verify that the BrowserWalletProvider correctly implements
//! all deezel-common traits and integrates properly with browser wallets.

use deezel_web::wallet_provider::{
    WalletConnector,
    WalletInfo as LocalWalletInfo, PsbtSigningOptions, PsbtSigningInput,
    WalletAccount, WalletConnectionStatus, WalletNetworkInfo
};
use deezel_web::provider::WebProvider;
use deezel_common::{DeezelProvider, LogProvider, TimeProvider, CryptoProvider, StorageProvider};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_wallet_info_creation() {
    let wallet_info = LocalWalletInfo {
        id: "test_wallet".to_string(),
        name: "Test Wallet".to_string(),
        icon: "https://example.com/icon.png".to_string(),
        website: "https://example.com".to_string(),
        injection_key: "testWallet".to_string(),
        supports_psbt: true,
        supports_taproot: true,
        supports_ordinals: false,
        mobile_support: true,
        deep_link_scheme: Some("testwallet://".to_string()),
    };
    
    assert_eq!(wallet_info.id, "test_wallet");
    assert_eq!(wallet_info.name, "Test Wallet");
    assert!(wallet_info.supports_psbt);
    assert!(wallet_info.supports_taproot);
    assert!(!wallet_info.supports_ordinals);
    assert!(wallet_info.mobile_support);
}

#[wasm_bindgen_test]
fn test_wallet_connector_creation() {
    let _connector = WalletConnector::new();
    
    // Test that supported wallets are properly initialized
    let supported_wallets = WalletConnector::get_supported_wallets();
    assert!(!supported_wallets.is_empty());
    
    // Check that common wallets are included
    let wallet_ids: Vec<&str> = supported_wallets.iter().map(|w| w.id.as_str()).collect();
    assert!(wallet_ids.contains(&"unisat"));
    assert!(wallet_ids.contains(&"xverse"));
    assert!(wallet_ids.contains(&"phantom"));
    assert!(wallet_ids.contains(&"okx"));
}

#[wasm_bindgen_test]
async fn test_wallet_detection() {
    let connector = WalletConnector::new();
    
    // This will return empty in test environment since no wallets are injected
    let detected_wallets = connector.detect_wallets().await;
    assert!(detected_wallets.is_ok());
    
    let wallets = detected_wallets.unwrap();
    // In test environment, no wallets should be detected
    assert_eq!(wallets.len(), 0);
}

#[wasm_bindgen_test]
fn test_wallet_info_lookup() {
    let connector = WalletConnector::new();
    
    // Test getting wallet info by ID
    let unisat_info = connector.get_wallet_info("unisat");
    assert!(unisat_info.is_some());
    assert_eq!(unisat_info.unwrap().name, "Unisat Wallet");
    
    let nonexistent_info = connector.get_wallet_info("nonexistent");
    assert!(nonexistent_info.is_none());
}

#[wasm_bindgen_test]
fn test_psbt_signing_options() {
    let options = PsbtSigningOptions {
        auto_finalized: true,
        to_sign_inputs: Some(vec![
            PsbtSigningInput {
                index: 0,
                address: Some("bc1q...".to_string()),
                sighash_types: Some(vec![1]),
                disable_tweaked_public_key: Some(false),
            }
        ]),
    };
    
    assert!(options.auto_finalized);
    assert!(options.to_sign_inputs.is_some());
    assert_eq!(options.to_sign_inputs.as_ref().unwrap().len(), 1);
}

#[wasm_bindgen_test]
fn test_wallet_account_creation() {
    let account = WalletAccount {
        address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
        public_key: Some("03...".to_string()),
        compressed_public_key: Some("02...".to_string()),
        address_type: "p2wpkh".to_string(),
    };
    
    assert!(account.address.starts_with("bc1q"));
    assert_eq!(account.address_type, "p2wpkh");
    assert!(account.public_key.is_some());
}

#[wasm_bindgen_test]
fn test_wallet_connection_status() {
    let status = WalletConnectionStatus::Connected;
    
    match status {
        WalletConnectionStatus::Connected => (),
        _ => panic!("Expected Connected status"),
    }
    
    let error_status = WalletConnectionStatus::Error("Test error".to_string());
    match error_status {
        WalletConnectionStatus::Error(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Expected Error status"),
    }
}

#[wasm_bindgen_test]
fn test_wallet_network_info() {
    let network_info = WalletNetworkInfo {
        network: "mainnet".to_string(),
        chain_id: Some("bitcoin".to_string()),
    };
    
    assert_eq!(network_info.network, "mainnet");
    assert!(network_info.chain_id.is_some());
}

// Mock tests for provider functionality (since we can't inject real wallets in tests)

#[wasm_bindgen_test]
async fn test_web_provider_creation() {
    let provider = WebProvider::new(
        "regtest".to_string(),
    ).await;
    
    assert!(provider.is_ok());
    let provider = provider.unwrap();
    assert_eq!(provider.provider_name(), "WebProvider");
}

#[wasm_bindgen_test]
async fn test_provider_trait_compatibility() {
    // Test that our provider implements the required traits
    let provider = WebProvider::new(
        "regtest".to_string(),
    ).await.unwrap();
    
    // Test TimeProvider
    let now_secs = provider.now_secs();
    let now_millis = provider.now_millis();
    assert!(now_secs > 0);
    assert!(now_millis > now_secs * 1000);
    
    // Test LogProvider
    provider.info("Test log message");
    provider.debug("Test debug message");
    provider.warn("Test warning message");
    provider.error("Test error message");
    
    // Test CryptoProvider
    let random_bytes = provider.random_bytes(32);
    assert!(random_bytes.is_ok());
    assert_eq!(random_bytes.unwrap().len(), 32);
    
    let test_data = b"hello world";
    let hash = provider.sha256(test_data);
    assert!(hash.is_ok());
    assert_eq!(hash.unwrap().len(), 32);
}

#[wasm_bindgen_test]
async fn test_storage_operations() {
    let provider = WebProvider::new(
        "regtest".to_string(),
    ).await.unwrap();
    
    let test_key = "test_key";
    let test_data = b"test data";
    
    // Test write
    let write_result = provider.write(test_key, test_data).await;
    assert!(write_result.is_ok());
    
    // Test exists
    let exists_result = provider.exists(test_key).await;
    assert!(exists_result.is_ok());
    assert!(exists_result.unwrap());
    
    // Test read
    let read_result = provider.read(test_key).await;
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), test_data);
    
    // Test delete
    let delete_result = provider.delete(test_key).await;
    assert!(delete_result.is_ok());
    
    // Test exists after delete
    let exists_after_delete = provider.exists(test_key).await;
    assert!(exists_after_delete.is_ok());
    assert!(!exists_after_delete.unwrap());
}

#[wasm_bindgen_test]
fn test_supported_wallets_completeness() {
    let supported_wallets = WalletConnector::get_supported_wallets();
    
    // Verify we have the expected wallets
    let expected_wallets = vec![
        "unisat", "xverse", "phantom", "okx", "leather", "magic_eden"
    ];
    
    for expected in expected_wallets {
        let found = supported_wallets.iter().any(|w| w.id == expected);
        assert!(found, "Expected wallet {expected} not found in supported wallets");
    }
    
    // Verify all wallets have required fields
    for wallet in &supported_wallets {
        assert!(!wallet.id.is_empty());
        assert!(!wallet.name.is_empty());
        assert!(!wallet.injection_key.is_empty());
        assert!(!wallet.website.is_empty());
        assert!(!wallet.icon.is_empty());
    }
}

#[wasm_bindgen_test]
fn test_wallet_capabilities() {
    let supported_wallets = WalletConnector::get_supported_wallets();
    
    // Test that Unisat has expected capabilities
    let unisat = supported_wallets.iter().find(|w| w.id == "unisat").unwrap();
    assert!(unisat.supports_psbt);
    assert!(unisat.supports_taproot);
    assert!(unisat.supports_ordinals);
    assert!(!unisat.mobile_support);
    assert!(unisat.deep_link_scheme.is_none());
    
    // Test that Xverse has expected capabilities
    let xverse = supported_wallets.iter().find(|w| w.id == "xverse").unwrap();
    assert!(xverse.supports_psbt);
    assert!(xverse.supports_taproot);
    assert!(xverse.supports_ordinals);
    assert!(xverse.mobile_support);
    assert!(xverse.deep_link_scheme.is_some());
    assert_eq!(xverse.deep_link_scheme.as_ref().unwrap(), "xverse://");
}

// Integration tests would go here, but they require actual wallet injection
// which isn't possible in the test environment. These would be tested
// manually or in a browser environment with actual wallet extensions.

#[wasm_bindgen_test]
async fn test_mock_browser_wallet_provider() {
    // This test verifies the structure without requiring actual wallet injection
    
    // Create a mock wallet info
    let wallet_info = LocalWalletInfo {
        id: "mock_wallet".to_string(),
        name: "Mock Wallet".to_string(),
        icon: "https://example.com/icon.png".to_string(),
        website: "https://example.com".to_string(),
        injection_key: "mockWallet".to_string(),
        supports_psbt: true,
        supports_taproot: true,
        supports_ordinals: true,
        mobile_support: false,
        deep_link_scheme: None,
    };
    
    // Verify wallet info structure
    assert_eq!(wallet_info.id, "mock_wallet");
    assert!(wallet_info.supports_psbt);
    
    // Test that we can create the connector
    let connector = WalletConnector::new();
    assert!(connector.get_wallet_info("unisat").is_some());
}

#[wasm_bindgen_test]
fn test_serialization() {
    // Test that our structs can be serialized/deserialized
    let wallet_info = LocalWalletInfo {
        id: "test".to_string(),
        name: "Test Wallet".to_string(),
        icon: "icon.png".to_string(),
        website: "https://test.com".to_string(),
        injection_key: "test".to_string(),
        supports_psbt: true,
        supports_taproot: false,
        supports_ordinals: true,
        mobile_support: false,
        deep_link_scheme: None,
    };
    
    let json = serde_json::to_string(&wallet_info);
    assert!(json.is_ok());
    
    let deserialized: std::result::Result<LocalWalletInfo, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok());
    
    let wallet = deserialized.unwrap();
    assert_eq!(wallet.id, "test");
    assert_eq!(wallet.name, "Test Wallet");
}