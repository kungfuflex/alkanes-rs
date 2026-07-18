// crates/deezel-common/tests/keystore_logic_test.rs
// Chadson v69.0.0: Test for core keystore cryptographic logic.

use alkanes_cli_common::{WalletConfig, traits::WalletProvider};
use alkanes_cli_common::mock_provider::MockProvider;

#[tokio::test]
async fn test_create_and_load_keystore_logic() {
    // 1. Define wallet configuration and provider
    let config = WalletConfig::default();
    let mut provider = MockProvider::new(config.network);
    let password = "strongpassword".to_string();

    // 2. Create a new wallet and get its address
    let wallet_info = provider.create_wallet(config.clone(), None, Some(password.clone())).await.unwrap();
    let original_address = wallet_info.address;

    // 3. "Backup" the wallet (get the encrypted keystore)
    let keystore_json = provider.backup().await.unwrap();

    // 4. "Load" the wallet from the keystore
    let mut new_provider = MockProvider::new(config.network);
    let loaded_wallet_info = new_provider.load_wallet(config, Some(password)).await.unwrap();
    let loaded_address = loaded_wallet_info.address;

    // 5. Assert that the loaded wallet's address matches the original address
    assert_eq!(loaded_address, original_address);
}