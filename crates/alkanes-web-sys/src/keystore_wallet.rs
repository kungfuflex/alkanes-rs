// Chadson v69.69: Systematic Task Completion
//
// This file implements the WalletBackend for a local keystore wallet.

use async_trait::async_trait;
use deezel_common::{*};
use bip39::{Mnemonic, Seed};
use bitcoin::{
    network::Network,
    bip32::{DerivationPath, Xpriv},
    Address,
    secp256k1::Secp256k1,
};
use core::str::FromStr;
use crate::wallet_provider::{WalletBackend, WalletInfo, WalletAccount, WalletNetworkInfo, PsbtSigningOptions};
use crate::keystore::Keystore;
use wasm_bindgen_futures::JsFuture;

pub struct KeystoreWallet {
    info: WalletInfo,
    keystore: Keystore,
    password: Option<String>,
}

impl KeystoreWallet {
    pub fn new(info: WalletInfo, keystore: Keystore, password: Option<String>) -> Self {
        Self { info, keystore, password }
    }
}

#[async_trait(?Send)]
impl WalletBackend for KeystoreWallet {
    fn get_info(&self) -> &WalletInfo {
        &self.info
    }

    async fn is_available(&self) -> bool {
        // The keystore wallet is always "available" if it has been instantiated.
        true
    }

    async fn connect(&self) -> Result<WalletAccount> {
        // "Connecting" to a keystore wallet means decrypting it to get the address.
        let promise = self.keystore.decrypt_mnemonic(self.password.as_deref().unwrap_or(""));
        let mnemonic_val = JsFuture::from(promise).await.map_err(|e| DeezelError::Wallet(format!("Failed to decrypt mnemonic: {:?}", e)))?;
        let mnemonic = mnemonic_val.as_string().ok_or_else(|| DeezelError::Wallet("Failed to get mnemonic string".to_string()))?;
        let mnemonic = Mnemonic::from_phrase(&mnemonic, bip39::Language::English).map_err(|e| DeezelError::Wallet(e.to_string()))?;
        let seed = Seed::new(&mnemonic, self.password.as_deref().unwrap_or(""));
        let secp = Secp256k1::new();
        let master_key = Xpriv::new_master(Network::Regtest, seed.as_bytes()).map_err(|e| DeezelError::Wallet(e.to_string()))?;
        let path = DerivationPath::from_str("m/84'/1'/0'/0/0").map_err(|e| DeezelError::Wallet(e.to_string()))?;
        let child_key = master_key.derive_priv(&secp, &path).map_err(|e| DeezelError::Wallet(e.to_string()))?;
        let public_key = child_key.private_key.public_key(&secp);
        let compressed_public_key = bitcoin::key::CompressedPublicKey(public_key);
        let address = Address::p2wpkh(&compressed_public_key, Network::Regtest);

        Ok(WalletAccount {
            address: address.to_string(),
            public_key: Some(public_key.to_string()),
            compressed_public_key: Some(public_key.to_string()),
            address_type: "p2wpkh".to_string(),
        })
    }

    async fn disconnect(&self) -> Result<()> {
        // No-op for keystore wallet
        Ok(())
    }

    async fn get_accounts(&self) -> Result<Vec<WalletAccount>> {
        let account = self.connect().await?;
        Ok(vec![account])
    }

    async fn get_network(&self) -> Result<WalletNetworkInfo> {
        Ok(WalletNetworkInfo {
            network: "regtest".to_string(),
            chain_id: None,
        })
    }

    async fn switch_network(&self, _network: &str) -> Result<()> {
        // Not supported for keystore wallet
        Err(DeezelError::NotImplemented("Switching networks is not supported for keystore wallets.".to_string()))
    }

    async fn sign_message(&self, _message: &str, _address: &str) -> Result<String> {
        Err(DeezelError::NotImplemented("sign_message is not yet implemented for keystore wallets.".to_string()))
    }

    async fn sign_psbt(&self, _psbt_hex: &str, _options: Option<PsbtSigningOptions>) -> Result<String> {
        Err(DeezelError::NotImplemented("sign_psbt is not yet implemented for keystore wallets.".to_string()))
    }

    async fn sign_psbts(&self, _psbt_hexs: Vec<String>, _options: Option<PsbtSigningOptions>) -> Result<Vec<String>> {
        Err(DeezelError::NotImplemented("sign_psbts is not yet implemented for keystore wallets.".to_string()))
    }

    async fn push_tx(&self, _tx_hex: &str) -> Result<String> {
        Err(DeezelError::NotImplemented("push_tx is not supported for keystore wallets.".to_string()))
    }

    async fn push_psbt(&self, _psbt_hex: &str) -> Result<String> {
        Err(DeezelError::NotImplemented("push_psbt is not supported for keystore wallets.".to_string()))
    }

    async fn get_public_key(&self) -> Result<String> {
        Err(DeezelError::NotImplemented("get_public_key is not yet implemented for keystore wallets.".to_string()))
    }

    async fn get_balance(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn get_inscriptions(&self, _cursor: Option<u32>, _size: Option<u32>) -> Result<serde_json::Value> {
        Err(DeezelError::NotImplemented("get_inscriptions is not supported for keystore wallets.".to_string()))
    }
}