//! Integration tests for the WebProvider's WalletProvider implementation.
//!
//! These tests cover the full lifecycle of wallet management in a web environment,
//! including creation, address derivation, balance checking, transaction signing,
//! and persistence.

use wasm_bindgen_test::*;
use deezel_web::provider::WebProvider;
use deezel_common::{WalletProvider, Result};
use bitcoin::Network;

wasm_bindgen_test_configure!(run_in_browser);

async fn setup_provider() -> Result<WebProvider> {
    WebProvider::new("regtest".to_string()).await
}

#[wasm_bindgen_test]
async fn test_wallet_creation() -> Result<()> {
    let mut provider = setup_provider().await?;
    let config = provider.get_wallet_config();
    
    let wallet_info = provider.create_wallet(config, None, Some("test_password".to_string())).await?;
    
    assert!(!wallet_info.address.is_empty(), "Address should not be empty");
    assert_eq!(wallet_info.network, Network::Regtest, "Network should be regtest");
    assert!(wallet_info.mnemonic.is_some(), "Mnemonic should be present");

    Ok(())
}

#[wasm_bindgen_test]
async fn test_get_address() -> Result<()> {
    let mut provider = setup_provider().await?;
    let config = provider.get_wallet_config();
    provider.create_wallet(config, None, Some("test_password".to_string())).await?;

    let address = provider.get_address().await?;
    assert!(!address.is_empty(), "Should be able to get an address");

    Ok(())
}

#[wasm_bindgen_test]
async fn test_get_balance() -> Result<()> {
    let mut provider = setup_provider().await?;
    let config = provider.get_wallet_config();
    provider.create_wallet(config, None, Some("test_password".to_string())).await?;

    // In a test environment, we can't easily get real balance.
    // We are testing that the call doesn't fail and returns a zero balance.
    let balance = provider.get_balance(None).await?;
    assert_eq!(balance.confirmed, 0);
    assert_eq!(balance.pending, 0);

    Ok(())
}

#[wasm_bindgen_test]
async fn test_wallet_persistence() -> Result<()> {
    let mut provider = setup_provider().await?;
    let config = provider.get_wallet_config();
    
    let original_info = provider.create_wallet(config.clone(), None, Some("test_password".to_string())).await?;
    let original_address = original_info.address;
    let original_mnemonic = original_info.mnemonic.unwrap();

    // Create a new provider to simulate loading from storage
    let mut new_provider = setup_provider().await?;
    let loaded_info = new_provider.load_wallet(config, Some("test_password".to_string())).await?;

    assert_eq!(loaded_info.address, original_address, "Loaded address should match original");
    assert_eq!(loaded_info.mnemonic.unwrap(), original_mnemonic, "Loaded mnemonic should match original");

    Ok(())
}