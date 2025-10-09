// Chadson v69.69: Systematic Task Completion
//
// This file implements the WalletBackend for a local keystore wallet.

use async_trait::async_trait;
use alkanes_cli_common::{*};
use bip39::{Mnemonic, Seed};
use bitcoin::{
    network::Network,
    bip32::{DerivationPath, Xpriv},
    Address,
    secp256k1::Secp256k1,
    key::CompressedPublicKey,
};
use core::convert::TryInto;
use core::str::FromStr;
use crate::wallet_provider::{WalletBackend, WalletInfo, WalletAccount, WalletNetworkInfo, PsbtSigningOptions, WalletFuture};
use alkanes_cli_common::provider::{EnrichedUtxo, AllBalances};
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
    fn is_available<'a>(&'a self) -> WalletFuture<'a, bool> {
        Box::pin(async move {
            // The keystore wallet is always "available" if it has been instantiated.
            Ok(true)
        })
    }

    fn connect<'a>(&'a self) -> WalletFuture<'a, WalletAccount> {
        Box::pin(async move {
            // "Connecting" to a keystore wallet means decrypting it to get the address.
            let promise = self.keystore.decrypt_mnemonic(self.password.as_deref().unwrap_or(""));
            let mnemonic_val = JsFuture::from(promise).await.map_err(|e| AlkanesError::Wallet(format!("Failed to decrypt mnemonic: {:?}", e)))?;
            let mnemonic = mnemonic_val.as_string().ok_or_else(|| AlkanesError::Wallet("Failed to get mnemonic string".to_string()))?;
            let mnemonic = Mnemonic::from_phrase(&mnemonic, bip39::Language::English).map_err(|e| AlkanesError::Wallet(e.to_string()))?;
            let seed = Seed::new(&mnemonic, self.password.as_deref().unwrap_or(""));
            let secp = Secp256k1::new();
            let master_key = Xpriv::new_master(Network::Regtest, seed.as_bytes()).map_err(|e| AlkanesError::Wallet(e.to_string()))?;
            let path = DerivationPath::from_str("m/84'/1'/0'/0/0").map_err(|e| AlkanesError::Wallet(e.to_string()))?;
            let child_key = master_key.derive_priv(&secp, &path).map_err(|e| AlkanesError::Wallet(e.to_string()))?;
            let public_key = child_key.private_key.public_key(&secp);
            let public_key_obj = bitcoin::PublicKey::new(public_key);
            let compressed_pk: CompressedPublicKey = public_key_obj.try_into().map_err(|_| AlkanesError::Wallet("Failed to create compressed public key".to_string()))?;
            let address = Address::p2wpkh(&compressed_pk, Network::Regtest);

            Ok(WalletAccount {
                address: address.to_string(),
                public_key: Some(public_key_obj.to_string()),
                compressed_public_key: Some(hex::encode(public_key_obj.to_bytes())),
                address_type: "p2wpkh".to_string(),
            })
        })
    }

    fn disconnect<'a>(&'a self) -> WalletFuture<'a, ()> {
        Box::pin(async move {
            // No-op for keystore wallet
            Ok(())
        })
    }

    fn get_accounts<'a>(&'a self) -> WalletFuture<'a, Vec<WalletAccount>> {
        Box::pin(async move {
            let account = self.connect().await?;
            Ok(vec![account])
        })
    }

    fn get_network<'a>(&'a self) -> WalletFuture<'a, WalletNetworkInfo> {
        Box::pin(async move {
            Ok(WalletNetworkInfo {
                network: "regtest".to_string(),
                chain_id: None,
            })
        })
    }

    fn switch_network<'a>(&'a self, _network: &'a str) -> WalletFuture<'a, ()> {
        Box::pin(async move {
            // Not supported for keystore wallet
            Err(AlkanesError::NotImplemented("Switching networks is not supported for keystore wallets.".to_string()))
        })
    }

    fn sign_message<'a>(&'a self, _message: &'a str, _address: &'a str) -> WalletFuture<'a, String> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("sign_message is not yet implemented for keystore wallets.".to_string()))
        })
    }

    fn sign_psbt<'a>(&'a self, _psbt_hex: &'a str, _options: Option<PsbtSigningOptions>) -> WalletFuture<'a, String> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("sign_psbt is not yet implemented for keystore wallets.".to_string()))
        })
    }

    fn sign_psbts<'a>(&'a self, _psbt_hexs: Vec<String>, _options: Option<PsbtSigningOptions>) -> WalletFuture<'a, Vec<String>> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("sign_psbts is not yet implemented for keystore wallets.".to_string()))
        })
    }

    fn push_tx<'a>(&'a self, _tx_hex: &'a str) -> WalletFuture<'a, String> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("push_tx is not supported for keystore wallets.".to_string()))
        })
    }

    fn push_psbt<'a>(&'a self, _psbt_hex: &'a str) -> WalletFuture<'a, String> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("push_psbt is not supported for keystore wallets.".to_string()))
        })
    }

    fn get_public_key<'a>(&'a self) -> WalletFuture<'a, String> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("get_public_key is not yet implemented for keystore wallets.".to_string()))
        })
    }

    fn get_balance<'a>(&'a self) -> WalletFuture<'a, Option<u64>> {
        Box::pin(async move {
            Ok(None)
        })
    }

    fn get_inscriptions<'a>(&'a self, _cursor: Option<u32>, _size: Option<u32>) -> WalletFuture<'a, serde_json::Value> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("get_inscriptions is not supported for keystore wallets.".to_string()))
        })
    }

    fn get_enriched_utxos<'a>(&'a self, _addresses: Option<Vec<String>>) -> WalletFuture<'a, Vec<EnrichedUtxo>> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("get_enriched_utxos is not supported for keystore wallets.".to_string()))
        })
    }

    fn get_all_balances<'a>(&'a self, _addresses: Option<Vec<String>>) -> WalletFuture<'a, AllBalances> {
        Box::pin(async move {
            Err(AlkanesError::NotImplemented("get_all_balances is not supported for keystore wallets.".to_string()))
        })
    }
}