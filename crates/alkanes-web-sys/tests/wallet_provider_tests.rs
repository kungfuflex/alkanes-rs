//! Tests for the browser wallet provider system
//!
//! These tests verify that the BrowserWalletProvider correctly implements
//! all deezel-common traits and integrates properly with browser wallets.

use alkanes_web_sys::wallet_provider::{
    WalletConnector,
    WalletInfo as LocalWalletInfo, PsbtSigningOptions, PsbtSigningInput,
    WalletAccount, WalletConnectionStatus, WalletNetworkInfo
};
use alkanes_web_sys::provider::WebProvider;
use alkanes_cli_common::{DeezelProvider, LogProvider, TimeProvider, CryptoProvider, StorageProvider};
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

// ============================================================================
// Tests for JsWalletAdapter and WasmBrowserWalletProvider
// ============================================================================

use wasm_bindgen::JsValue;
use js_sys::{Promise, Function, Object, Reflect};

/// Helper to create a mock JS wallet adapter object for testing
fn create_mock_js_adapter() -> JsValue {
    let obj = Object::new();

    // Create getInfo function
    let get_info = Function::new_no_args(r#"
        return {
            id: 'mock',
            name: 'Mock Wallet',
            icon: '/mock.svg',
            website: 'https://mock.test',
            injection_key: 'mockWallet',
            supports_psbt: true,
            supports_taproot: true,
            supports_ordinals: true,
            mobile_support: false
        };
    "#);
    Reflect::set(&obj, &JsValue::from_str("getInfo"), &get_info).unwrap();

    // Create connect function
    let connect = Function::new_no_args(r#"
        return Promise.resolve({
            address: 'bc1qmock1234567890abcdef',
            public_key: '03' + '0'.repeat(64),
            address_type: 'p2wpkh'
        });
    "#);
    Reflect::set(&obj, &JsValue::from_str("connect"), &connect).unwrap();

    // Create disconnect function
    let disconnect = Function::new_no_args(r#"
        return Promise.resolve();
    "#);
    Reflect::set(&obj, &JsValue::from_str("disconnect"), &disconnect).unwrap();

    // Create getAccounts function
    let get_accounts = Function::new_no_args(r#"
        return Promise.resolve([{
            address: 'bc1qmock1234567890abcdef',
            public_key: '03' + '0'.repeat(64),
            address_type: 'p2wpkh'
        }]);
    "#);
    Reflect::set(&obj, &JsValue::from_str("getAccounts"), &get_accounts).unwrap();

    // Create getNetwork function
    let get_network = Function::new_no_args(r#"
        return Promise.resolve('mainnet');
    "#);
    Reflect::set(&obj, &JsValue::from_str("getNetwork"), &get_network).unwrap();

    // Create switchNetwork function
    let switch_network = Function::new_with_args("network", r#"
        return Promise.resolve();
    "#);
    Reflect::set(&obj, &JsValue::from_str("switchNetwork"), &switch_network).unwrap();

    // Create signMessage function
    let sign_message = Function::new_with_args("message, address", r#"
        return Promise.resolve('mock_signature_base64');
    "#);
    Reflect::set(&obj, &JsValue::from_str("signMessage"), &sign_message).unwrap();

    // Create signPsbt function
    let sign_psbt = Function::new_with_args("psbtHex, options", r#"
        // Just return the same hex for testing
        return Promise.resolve(psbtHex);
    "#);
    Reflect::set(&obj, &JsValue::from_str("signPsbt"), &sign_psbt).unwrap();

    // Create signPsbts function
    let sign_psbts = Function::new_with_args("psbtHexs, options", r#"
        return Promise.resolve(psbtHexs);
    "#);
    Reflect::set(&obj, &JsValue::from_str("signPsbts"), &sign_psbts).unwrap();

    // Create pushTx function
    let push_tx = Function::new_with_args("txHex", r#"
        return Promise.resolve('0'.repeat(64));
    "#);
    Reflect::set(&obj, &JsValue::from_str("pushTx"), &push_tx).unwrap();

    // Create pushPsbt function
    let push_psbt = Function::new_with_args("psbtHex", r#"
        return Promise.resolve('0'.repeat(64));
    "#);
    Reflect::set(&obj, &JsValue::from_str("pushPsbt"), &push_psbt).unwrap();

    // Create getPublicKey function
    let get_public_key = Function::new_no_args(r#"
        return Promise.resolve('03' + '0'.repeat(64));
    "#);
    Reflect::set(&obj, &JsValue::from_str("getPublicKey"), &get_public_key).unwrap();

    // Create getBalance function
    let get_balance = Function::new_no_args(r#"
        return Promise.resolve(100000000);
    "#);
    Reflect::set(&obj, &JsValue::from_str("getBalance"), &get_balance).unwrap();

    // Create getInscriptions function
    let get_inscriptions = Function::new_with_args("cursor, size", r#"
        return Promise.resolve({ list: [], total: 0 });
    "#);
    Reflect::set(&obj, &JsValue::from_str("getInscriptions"), &get_inscriptions).unwrap();

    obj.into()
}

#[wasm_bindgen_test]
fn test_mock_adapter_info() {
    let adapter_js = create_mock_js_adapter();

    // Verify getInfo returns expected structure
    let get_info = Reflect::get(&adapter_js, &JsValue::from_str("getInfo")).unwrap();
    let get_info_fn = Function::from(get_info);
    let info = get_info_fn.call0(&adapter_js).unwrap();

    let id = Reflect::get(&info, &JsValue::from_str("id")).unwrap();
    assert_eq!(id.as_string().unwrap(), "mock");

    let name = Reflect::get(&info, &JsValue::from_str("name")).unwrap();
    assert_eq!(name.as_string().unwrap(), "Mock Wallet");

    let supports_psbt = Reflect::get(&info, &JsValue::from_str("supports_psbt")).unwrap();
    assert!(supports_psbt.as_bool().unwrap());
}

#[wasm_bindgen_test]
async fn test_mock_adapter_connect() {
    let adapter_js = create_mock_js_adapter();

    // Test connect function
    let connect = Reflect::get(&adapter_js, &JsValue::from_str("connect")).unwrap();
    let connect_fn = Function::from(connect);
    let promise = connect_fn.call0(&adapter_js).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();

    let address = Reflect::get(&result, &JsValue::from_str("address")).unwrap();
    assert!(address.as_string().unwrap().starts_with("bc1q"));

    let address_type = Reflect::get(&result, &JsValue::from_str("address_type")).unwrap();
    assert_eq!(address_type.as_string().unwrap(), "p2wpkh");
}

#[wasm_bindgen_test]
async fn test_mock_adapter_sign_message() {
    let adapter_js = create_mock_js_adapter();

    // Test signMessage function
    let sign_message = Reflect::get(&adapter_js, &JsValue::from_str("signMessage")).unwrap();
    let sign_message_fn = Function::from(sign_message);
    let promise = sign_message_fn.call2(
        &adapter_js,
        &JsValue::from_str("Hello, World!"),
        &JsValue::from_str("bc1qtest")
    ).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    assert!(result.is_string());
    assert_eq!(result.as_string().unwrap(), "mock_signature_base64");
}

#[wasm_bindgen_test]
async fn test_mock_adapter_get_network() {
    let adapter_js = create_mock_js_adapter();

    // Test getNetwork function
    let get_network = Reflect::get(&adapter_js, &JsValue::from_str("getNetwork")).unwrap();
    let get_network_fn = Function::from(get_network);
    let promise = get_network_fn.call0(&adapter_js).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    assert_eq!(result.as_string().unwrap(), "mainnet");
}

#[wasm_bindgen_test]
async fn test_mock_adapter_get_balance() {
    let adapter_js = create_mock_js_adapter();

    // Test getBalance function
    let get_balance = Reflect::get(&adapter_js, &JsValue::from_str("getBalance")).unwrap();
    let get_balance_fn = Function::from(get_balance);
    let promise = get_balance_fn.call0(&adapter_js).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    assert_eq!(result.as_f64().unwrap(), 100000000.0);
}

#[wasm_bindgen_test]
async fn test_mock_adapter_sign_psbt() {
    let adapter_js = create_mock_js_adapter();

    // Test signPsbt function with a mock PSBT hex
    let mock_psbt_hex = "70736274ff01003f0200000001...";

    let sign_psbt = Reflect::get(&adapter_js, &JsValue::from_str("signPsbt")).unwrap();
    let sign_psbt_fn = Function::from(sign_psbt);
    let promise = sign_psbt_fn.call2(
        &adapter_js,
        &JsValue::from_str(mock_psbt_hex),
        &JsValue::undefined()
    ).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
    // The mock just returns the same hex
    assert_eq!(result.as_string().unwrap(), mock_psbt_hex);
}

#[wasm_bindgen_test]
async fn test_mock_adapter_get_inscriptions() {
    let adapter_js = create_mock_js_adapter();

    // Test getInscriptions function
    let get_inscriptions = Reflect::get(&adapter_js, &JsValue::from_str("getInscriptions")).unwrap();
    let get_inscriptions_fn = Function::from(get_inscriptions);
    let promise = get_inscriptions_fn.call2(
        &adapter_js,
        &JsValue::from_f64(0.0),
        &JsValue::from_f64(20.0)
    ).unwrap();
    let promise = Promise::from(promise);

    let result = wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();

    let total = Reflect::get(&result, &JsValue::from_str("total")).unwrap();
    assert_eq!(total.as_f64().unwrap(), 0.0);
}

// Tests for wallet adapter compatibility
#[wasm_bindgen_test]
fn test_wallet_adapter_interface_completeness() {
    let adapter_js = create_mock_js_adapter();

    // Verify all required methods exist
    let required_methods = vec![
        "getInfo",
        "connect",
        "disconnect",
        "getAccounts",
        "getNetwork",
        "switchNetwork",
        "signMessage",
        "signPsbt",
        "signPsbts",
        "pushTx",
        "pushPsbt",
        "getPublicKey",
        "getBalance",
        "getInscriptions",
    ];

    for method in required_methods {
        let has_method = Reflect::has(&adapter_js, &JsValue::from_str(method)).unwrap();
        assert!(has_method, "Missing required method: {method}");
    }
}

#[wasm_bindgen_test]
fn test_psbt_signing_input_serialization() {
    let input = PsbtSigningInput {
        index: 0,
        address: Some("bc1qtest".to_string()),
        sighash_types: Some(vec![1, 2]),
        disable_tweaked_public_key: Some(false),
    };

    let json = serde_json::to_string(&input).unwrap();
    assert!(json.contains("\"index\":0"));
    assert!(json.contains("\"address\":\"bc1qtest\""));

    let deserialized: PsbtSigningInput = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.index, 0);
    assert_eq!(deserialized.address.unwrap(), "bc1qtest");
}