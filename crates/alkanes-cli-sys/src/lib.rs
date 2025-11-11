//! Deezel System Library
//!
//! This library provides the system-level implementation of the deezel CLI,
//! acting as a bridge between the command-line interface and the deezel-common
//! library. It is designed to be used as a library by system crates that
//! utilize alkanes on the backend.

use anyhow::{anyhow, Context};
use alkanes_cli_common::{Result, AlkanesError};
use async_trait::async_trait;
use alkanes_cli_common::provider::ConcreteProvider;
use alkanes_cli_common::traits::*;
use alkanes_cli_common::commands::*;

pub mod utils;
pub mod keystore;
pub mod pretty_print;
use alkanes_cli_common::alkanes::AlkanesInspectConfig;
use utils::*;
use keystore::{KeystoreManager, KeystoreCreateParams};

pub struct SystemAlkanes {
    provider: ConcreteProvider,
    keystore_manager: KeystoreManager,
    args: Args,
}

impl SystemAlkanes {
    pub async fn new(args: &Args) -> anyhow::Result<Self> {
        // Determine network parameters based on provider and magic flags
        let mut network_params = if let Some(magic_str) = args.magic.as_ref() {
            // Parse custom magic bytes
            match alkanes_cli_common::network::NetworkParams::from_magic_str(magic_str) {
                Ok((p2pkh_prefix, p2sh_prefix, bech32_hrp)) => {
                    // Use the base network from provider and apply custom magic bytes
                    let base_network = match args.rpc_config.provider.as_str() {
                        "mainnet" => bitcoin::Network::Bitcoin,
                        "testnet" => bitcoin::Network::Testnet,
                        "signet" => bitcoin::Network::Signet,
                        "regtest" => bitcoin::Network::Regtest,
                        _ => bitcoin::Network::Regtest,
                    };
                    alkanes_cli_common::network::NetworkParams::with_custom_magic(
                        base_network,
                        p2pkh_prefix,
                        p2sh_prefix,
                        bech32_hrp,
                    )
                },
                Err(e) => {
                    eprintln!("⚠️  Invalid magic bytes format: {e}");
                    eprintln!("💡 Expected format: p2pkh_prefix,p2sh_prefix,bech32_hrp (e.g., '0x00,0x05,bc')");
                    return Err(anyhow!("Invalid magic bytes: {}", e));
                }
            }
        } else {
            // Use predefined network parameters
            match alkanes_cli_common::network::NetworkParams::from_network_str(&args.rpc_config.provider) {
                Ok(params) => params,
                Err(_) => {
                    eprintln!("⚠️  Unknown network: {}", args.rpc_config.provider);
                    eprintln!("💡 Supported networks: {}", alkanes_cli_common::network::NetworkParams::supported_networks().join(", "));
                    alkanes_cli_common::network::NetworkParams::regtest() // Default fallback
                }
            }
        };

        // If a bitcoin_rpc_url is provided and the network is regtest, override the default.
        if let Some(rpc_url) = &args.rpc_config.bitcoin_rpc_url {
            if network_params.network == bitcoin::Network::Regtest {
                network_params.bitcoin_rpc_url = rpc_url.clone();
                network_params.metashrew_rpc_url = rpc_url.clone();
                network_params.esplora_url = Some(rpc_url.clone());
            }
        }

        // Handle wallet-address mode (no keystore needed)
        let wallet_path_opt = if args.wallet_address.is_some() {
            // In address-only mode, we don't need a wallet file
            None
        } else if let Some(ref path) = args.wallet_file {
            Some(expand_tilde(path)?)
        } else {
            let network_name = match network_params.network {
                bitcoin::Network::Bitcoin => "mainnet",
                bitcoin::Network::Testnet => "testnet",
                bitcoin::Network::Signet => "signet",
                bitcoin::Network::Regtest => "regtest",
                _ => "custom",
            };
            // Default to keystore.json extension (not .asc since we handle encryption internally)
            Some(expand_tilde(&format!("~/.deezel/{network_name}.keystore.json"))?)
        };
        
        // Create wallet directory if it doesn't exist
        if let Some(ref wallet_file) = wallet_path_opt {
            if let Some(parent) = std::path::Path::new(wallet_file).parent() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create wallet directory")?;
            }
        }

        // Determine the correct RPC URLs, prioritizing command-line args over network defaults.
        // Only use the network default bitcoin_rpc_url if sandshrew_rpc_url is not provided
        let bitcoin_rpc_url = args
            .rpc_config
            .bitcoin_rpc_url
            .clone()
            .or_else(|| {
                if args.rpc_config.sandshrew_rpc_url.is_none() {
                    Some(network_params.bitcoin_rpc_url.clone())
                } else {
                    None
                }
            });

        let metashrew_rpc_url = args
            .rpc_config
            .metashrew_rpc_url
            .clone()
            .or_else(|| args.rpc_config.sandshrew_rpc_url.clone())
            .unwrap_or_else(|| network_params.metashrew_rpc_url.clone());

        let esplora_url = args
            .rpc_config
            .esplora_url
            .clone()
            .or_else(|| network_params.esplora_url.clone());

        // Create provider with the resolved URLs
        log::info!(
            "Creating ConcreteProvider with URLs: bitcoin_rpc: {:?}, metashrew_rpc: {:?}, sandshrew_rpc: {:?}, esplora: {:?}",
            &bitcoin_rpc_url,
            &metashrew_rpc_url,
            &args.rpc_config.sandshrew_rpc_url,
            &esplora_url
        );
        let mut provider = ConcreteProvider::new(
            bitcoin_rpc_url,
            metashrew_rpc_url,
            args.rpc_config.sandshrew_rpc_url.clone(),
            esplora_url,
            args.rpc_config.provider.clone(),
            wallet_path_opt.map(std::path::PathBuf::from),
        )
        .await?;

        if let Some(passphrase) = &args.passphrase {
            log::debug!("Setting passphrase for wallet");
            provider.set_passphrase(Some(passphrase.clone()));
        } else {
            log::debug!("No passphrase provided");
        }

        // Handle different wallet modes
        if let Some(ref address) = args.wallet_address {
            // Address-only mode: no keystore needed
            log::info!("Using address-only mode with address: {}", address);
            provider.set_address_only_mode(address.clone(), "p2wpkh".to_string());
        } else if let Some(ref key_file) = args.wallet_key_file {
            // External key mode: load private key from file
            log::info!("Loading private key from file: {}", key_file);
            provider.load_external_key(key_file)?;
        } else {
            // Normal keystore mode
            provider.initialize().await?;
        }

        // Create PGP provider

        // Create keystore manager
        let keystore_manager = KeystoreManager::new();

        Ok(Self {
            provider,
            keystore_manager,
            args: args.clone(),
        })
    }
}

#[async_trait(?Send)]
impl System for SystemAlkanes {
    fn provider(&self) -> &dyn DeezelProvider {
        &self.provider
    }

    fn provider_mut(&mut self) -> &mut dyn DeezelProvider {
        &mut self.provider
    }
}

#[async_trait(?Send)]
impl DeezelProvider for SystemAlkanes {
    fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }

    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        self.provider.get_bitcoin_rpc_url()
    }

    fn get_esplora_api_url(&self) -> Option<String> {
        self.provider.get_esplora_api_url()
    }

    fn get_ord_server_url(&self) -> Option<String> {
        self.provider.get_ord_server_url()
    }

    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }

    fn get_metashrew_rpc_url(&self) -> Option<String> {
        unimplemented!()
    }

    async fn wrap(&mut self, _amount: u64, _address: Option<String>, _fee_rate: Option<f32>) -> Result<String> {
        unimplemented!()
    }

    async fn unwrap(&mut self, _amount: u64, _address: Option<String>) -> Result<String> {
        unimplemented!()
    }

    async fn initialize(&self) -> Result<()> {
        self.provider.initialize().await
    }

    async fn shutdown(&self) -> Result<()> {
        self.provider.shutdown().await
    }

    fn secp(&self) -> &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All> {
        self.provider.secp()
    }

    async fn get_utxo(&self, outpoint: &bitcoin::OutPoint) -> Result<Option<bitcoin::TxOut>> {
        self.provider.get_utxo(outpoint).await
    }

    async fn sign_taproot_script_spend(
        &self,
        sighash: bitcoin::secp256k1::Message,
    ) -> Result<bitcoin::secp256k1::schnorr::Signature> {
        self.provider.sign_taproot_script_spend(sighash).await
    }
}

#[async_trait(?Send)]
impl JsonRpcProvider for SystemAlkanes {
    async fn call(&self, url: &str, method: &str, params: alkanes_cli_common::JsonValue, id: u64) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.call(url, method, params, id).await
    }
}

#[async_trait(?Send)]
impl StorageProvider for SystemAlkanes {
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        self.provider.read(key).await
    }
    async fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        self.provider.write(key, data).await
    }
    async fn exists(&self, key: &str) -> Result<bool> {
        self.provider.exists(key).await
    }
    async fn delete(&self, key: &str) -> Result<()> {
        self.provider.delete(key).await
    }
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        self.provider.list_keys(prefix).await
    }
    fn storage_type(&self) -> &'static str {
        self.provider.storage_type()
    }
}

#[async_trait(?Send)]
impl NetworkProvider for SystemAlkanes {
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.provider.get(url).await
    }
    async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
        self.provider.post(url, body, content_type).await
    }
    async fn is_reachable(&self, url: &str) -> bool {
        self.provider.is_reachable(url).await
    }
}

#[async_trait(?Send)]
impl CryptoProvider for SystemAlkanes {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>> {
        self.provider.random_bytes(len)
    }
    fn sha256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.provider.sha256(data)
    }
    fn sha3_256(&self, data: &[u8]) -> Result<[u8; 32]> {
        self.provider.sha3_256(data)
    }
    async fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.provider.encrypt_aes_gcm(data, key, nonce).await
    }
    async fn decrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        self.provider.decrypt_aes_gcm(data, key, nonce).await
    }
    async fn pbkdf2_derive(&self, password: &[u8], salt: &[u8], iterations: u32, key_len: usize) -> Result<Vec<u8>> {
        self.provider.pbkdf2_derive(password, salt, iterations, key_len).await
    }
}

#[async_trait(?Send)]
impl TimeProvider for SystemAlkanes {
    fn now_secs(&self) -> u64 {
        self.provider.now_secs()
    }
    fn now_millis(&self) -> u64 {
        self.provider.now_millis()
    }
    async fn sleep_ms(&self, ms: u64) {
        self.provider.sleep_ms(ms).await
    }
}

impl LogProvider for SystemAlkanes {
    fn debug(&self, message: &str) {
        self.provider.debug(message)
    }
    fn info(&self, message: &str) {
        self.provider.info(message)
    }
    fn warn(&self, message: &str) {
        self.provider.warn(message)
    }
    fn error(&self, message: &str) {
        self.provider.error(message)
    }
}

#[async_trait(?Send)]
impl WalletProvider for SystemAlkanes {
    async fn create_wallet(&mut self, config: WalletConfig, mnemonic: Option<String>, passphrase: Option<String>) -> Result<WalletInfo> {
        self.provider.create_wallet(config, mnemonic, passphrase).await
    }
    async fn load_wallet(&mut self, config: WalletConfig, passphrase: Option<String>) -> Result<WalletInfo> {
        self.provider.load_wallet(config, passphrase).await
    }
    async fn get_balance(&self, addresses: Option<Vec<String>>) -> Result<WalletBalance> {
        <ConcreteProvider as WalletProvider>::get_balance(&self.provider, addresses).await
    }
    async fn get_address(&self) -> Result<String> {
        <ConcreteProvider as WalletProvider>::get_address(&self.provider).await
    }
    async fn get_addresses(&self, count: u32) -> Result<Vec<alkanes_cli_common::traits::AddressInfo>> {
        self.provider.get_addresses(count).await
    }
    async fn send(&mut self, params: SendParams) -> Result<String> {
        self.provider.send(params).await
    }
    async fn get_utxos(&self, include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<(bitcoin::OutPoint, alkanes_cli_common::traits::UtxoInfo)>> {
        self.provider.get_utxos(include_frozen, addresses).await
    }
    async fn get_history(&self, count: u32, address: Option<String>) -> Result<Vec<TransactionInfo>> {
        self.provider.get_history(count, address).await
    }
    async fn freeze_utxo(&self, utxo: String, reason: Option<String>) -> Result<()> {
        self.provider.freeze_utxo(utxo, reason).await
    }
    async fn unfreeze_utxo(&self, utxo: String) -> Result<()> {
        self.provider.unfreeze_utxo(utxo).await
    }
    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        self.provider.create_transaction(params).await
    }
    async fn sign_transaction(&mut self, tx_hex: String) -> Result<String> {
        self.provider.sign_transaction(tx_hex).await
    }
    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        self.provider.broadcast_transaction(tx_hex).await
    }
    async fn estimate_fee(&self, target: u32) -> Result<FeeEstimate> {
        self.provider.estimate_fee(target).await
    }
    async fn get_fee_rates(&self) -> Result<FeeRates> {
        self.provider.get_fee_rates().await
    }
    async fn sync(&self) -> Result<()> {
        self.provider.sync().await
    }
    async fn backup(&self) -> Result<String> {
        self.provider.backup().await
    }
    async fn get_mnemonic(&self) -> Result<Option<String>> {
        self.provider.get_mnemonic().await
    }
    fn get_network(&self) -> bitcoin::Network {
        self.provider.get_network()
    }
    async fn get_internal_key(&self) -> Result<(bitcoin::XOnlyPublicKey, (bitcoin::bip32::Fingerprint, bitcoin::bip32::DerivationPath))> {
        self.provider.get_internal_key().await
    }
    async fn sign_psbt(&mut self, psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> {
        self.provider.sign_psbt(psbt).await
    }
    async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> {
        self.provider.get_keypair().await
    }
    fn set_passphrase(&mut self, passphrase: Option<String>) {
        self.provider.set_passphrase(passphrase)
    }
    async fn get_last_used_address_index(&self) -> Result<u32> {
        self.provider.get_last_used_address_index().await
    }

    async fn get_master_public_key(&self) -> Result<Option<String>> {
        unimplemented!()
    }

    async fn get_enriched_utxos(&self, _addresses: Option<Vec<String>>) -> Result<Vec<alkanes_cli_common::provider::EnrichedUtxo>> {
        unimplemented!()
    }

    async fn get_all_balances(&self, _addresses: Option<Vec<String>>) -> Result<alkanes_cli_common::provider::AllBalances> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl AddressResolver for SystemAlkanes {
    async fn resolve_all_identifiers(&self, input: &str) -> Result<String> {
        self.provider.resolve_all_identifiers(input).await
    }
    fn contains_identifiers(&self, input: &str) -> bool {
        self.provider.contains_identifiers(input)
    }
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        <ConcreteProvider as AddressResolver>::get_address(&self.provider, address_type, index).await
    }
    async fn list_identifiers(&self) -> Result<Vec<String>> {
        self.provider.list_identifiers().await
    }
}

#[async_trait(?Send)]
impl BitcoinRpcProvider for SystemAlkanes {
    async fn get_block_count(&self) -> Result<u64> {
        self.provider.get_block_count().await
    }
    async fn generate_to_address(&self, nblocks: u32, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.generate_to_address(nblocks, address).await
    }
    async fn get_blockchain_info(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_blockchain_info().await
    }
    async fn get_new_address(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_new_address().await
    }
    async fn get_transaction_hex(&self, txid: &str) -> Result<String> {
        self.provider.get_transaction_hex(txid).await
    }
    async fn get_block(&self, hash: &str, raw: bool) -> Result<alkanes_cli_common::JsonValue> {
        <ConcreteProvider as BitcoinRpcProvider>::get_block(&self.provider, hash, raw).await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        <ConcreteProvider as BitcoinRpcProvider>::get_block_hash(&self.provider, height).await
    }
    async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        self.provider.send_raw_transaction(tx_hex).await
    }
    async fn get_mempool_info(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_mempool_info().await
    }
    async fn estimate_smart_fee(&self, target: u32) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.estimate_smart_fee(target).await
    }
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> {
        self.provider.get_esplora_blocks_tip_height().await
    }
    async fn trace_transaction(&self, txid: &str, vout: u32, block: Option<&str>, tx: Option<&str>) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.trace_transaction(txid, vout, block, tx).await
    }

    async fn get_network_info(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_network_info().await
    }

    async fn get_raw_transaction(&self, txid: &str, block_hash: Option<&str>) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_raw_transaction(txid, block_hash).await
    }

    async fn get_block_header(&self, hash: &str) -> Result<alkanes_cli_common::JsonValue> {
        alkanes_cli_common::BitcoinRpcProvider::get_block_header(&self.provider, hash).await
    }

    async fn get_block_stats(&self, hash: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_block_stats(hash).await
    }

    async fn get_chain_tips(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_chain_tips().await
    }

    async fn get_raw_mempool(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_raw_mempool().await
    }

    async fn get_tx_out(&self, txid: &str, vout: u32, include_mempool: bool) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx_out(txid, vout, include_mempool).await
    }
}

#[async_trait(?Send)]
impl MetashrewRpcProvider for SystemAlkanes {
    async fn get_metashrew_height(&self) -> Result<u64> {
        self.provider.get_metashrew_height().await
    }
    async fn get_state_root(&self, height: alkanes_cli_common::JsonValue) -> Result<String> {
        alkanes_cli_common::MetashrewRpcProvider::get_state_root(self, height).await
    }
    async fn get_contract_meta(&self, block: &str, tx: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_contract_meta(block, tx).await
    }
    async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.trace_outpoint(txid, vout).await
    }
    async fn get_spendables_by_address(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_spendables_by_address(address).await
    }
    async fn get_protorunes_by_address(&self, address: &str, block_tag: Option<String>, protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneWalletResponse> {
        self.provider.get_protorunes_by_address(address, block_tag, protocol_tag).await
    }
    async fn get_protorunes_by_outpoint(&self, txid: &str, vout: u32, block_tag: Option<String>, protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneOutpointResponse> {
        self.provider.get_protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }
}

#[async_trait(?Send)]
impl MetashrewProvider for SystemAlkanes {
    async fn get_height(&self) -> Result<u64> {
        self.provider.get_height().await
    }
    async fn get_block_hash(&self, height: u64) -> Result<String> {
        <ConcreteProvider as MetashrewProvider>::get_block_hash(&self.provider, height).await
    }
    async fn get_state_root(&self, height: alkanes_cli_common::JsonValue) -> Result<String> {
        alkanes_cli_common::MetashrewProvider::get_state_root(self, height).await
    }
}

#[async_trait(?Send)]
impl EsploraProvider for SystemAlkanes {
    async fn get_blocks_tip_hash(&self) -> Result<String> {
        self.provider.get_blocks_tip_hash().await
    }
    async fn get_blocks_tip_height(&self) -> Result<u64> {
        self.provider.get_blocks_tip_height().await
    }
    async fn get_blocks(&self, start_height: Option<u64>) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_blocks(start_height).await
    }
    async fn get_block_by_height(&self, height: u64) -> Result<String> {
        self.provider.get_block_by_height(height).await
    }
    async fn get_block(&self, hash: &str) -> Result<alkanes_cli_common::JsonValue> {
        <ConcreteProvider as EsploraProvider>::get_block(&self.provider, hash).await
    }
    async fn get_block_status(&self, hash: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_block_status(hash).await
    }
    async fn get_block_txids(&self, hash: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_block_txids(hash).await
    }
    async fn get_block_header(&self, hash: &str) -> Result<String> {
        alkanes_cli_common::EsploraProvider::get_block_header(&self.provider, hash).await
    }
    async fn get_block_raw(&self, hash: &str) -> Result<String> {
        self.provider.get_block_raw(hash).await
    }
    async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String> {
        self.provider.get_block_txid(hash, index).await
    }
    async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_block_txs(hash, start_index).await
    }
    async fn get_address_info(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_info(address).await
    }
    async fn get_address_txs(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_txs(address).await
    }
    async fn get_address_txs_chain(&self, address: &str, last_seen_txid: Option<&str>) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_txs_chain(address, last_seen_txid).await
    }
    async fn get_address_txs_mempool(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_txs_mempool(address).await
    }
    async fn get_address_utxo(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_utxo(address).await
    }
    async fn get_address_prefix(&self, prefix: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_address_prefix(prefix).await
    }
    async fn get_tx(&self, txid: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx(txid).await
    }
    async fn get_tx_hex(&self, txid: &str) -> Result<String> {
        self.provider.get_tx_hex(txid).await
    }
    async fn get_tx_raw(&self, txid: &str) -> Result<String> {
        self.provider.get_tx_raw(txid).await
    }
    async fn get_tx_status(&self, txid: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx_status(txid).await
    }
    async fn get_tx_merkle_proof(&self, txid: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx_merkle_proof(txid).await
    }
    async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String> {
        self.provider.get_tx_merkleblock_proof(txid).await
    }
    async fn get_tx_outspend(&self, txid: &str, index: u32) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx_outspend(txid, index).await
    }
    async fn get_tx_outspends(&self, txid: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_tx_outspends(txid).await
    }
    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        self.provider.broadcast(tx_hex).await
    }
    async fn get_mempool(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_mempool().await
    }
    async fn get_mempool_txids(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_mempool_txids().await
    }
    async fn get_mempool_recent(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_mempool_recent().await
    }
    async fn get_fee_estimates(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.get_fee_estimates().await
    }
}

#[async_trait(?Send)]
impl RunestoneProvider for SystemAlkanes {
    async fn decode_runestone(&self, tx: &bitcoin::Transaction) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.decode_runestone(tx).await
    }
    async fn format_runestone_with_decoded_messages(&self, tx: &bitcoin::Transaction) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.format_runestone_with_decoded_messages(tx).await
    }
    async fn analyze_runestone(&self, txid: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.analyze_runestone(txid).await
    }
}

#[async_trait(?Send)]
impl AlkanesProvider for SystemAlkanes {
    async fn execute(&mut self, params: alkanes_cli_common::alkanes::types::EnhancedExecuteParams) -> Result<alkanes_cli_common::alkanes::types::ExecutionState> {
        self.provider.execute(params).await
    }
    async fn resume_execution(&mut self, state: alkanes_cli_common::alkanes::types::ReadyToSignTx, params: &alkanes_cli_common::alkanes::types::EnhancedExecuteParams) -> Result<alkanes_cli_common::alkanes::types::EnhancedExecuteResult> {
        self.provider.resume_execution(state, params).await
    }
    async fn resume_commit_execution(&mut self, state: alkanes_cli_common::alkanes::types::ReadyToSignCommitTx) -> Result<alkanes_cli_common::alkanes::types::ExecutionState> {
        self.provider.resume_commit_execution(state).await
    }
    async fn resume_reveal_execution(&mut self, state: alkanes_cli_common::alkanes::types::ReadyToSignRevealTx) -> Result<alkanes_cli_common::alkanes::types::EnhancedExecuteResult> {
        self.provider.resume_reveal_execution(state).await
    }
    async fn protorunes_by_address(&self, address: &str, block_tag: Option<String>, protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneWalletResponse> {
        self.provider.protorunes_by_address(address, block_tag, protocol_tag).await
    }
    async fn protorunes_by_outpoint(&self, txid: &str, vout: u32, block_tag: Option<String>, protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneOutpointResponse> {
        self.provider.protorunes_by_outpoint(txid, vout, block_tag, protocol_tag).await
    }
    async fn view(&self, _contract_id: &str, _view_fn: &str, _params: Option<&[u8]>) -> Result<alkanes_cli_common::JsonValue> {
        unimplemented!()
    }

    async fn simulate(&self, contract_id: &str, context: &alkanes_cli_common::proto::alkanes::MessageContextParcel) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.simulate(contract_id, context).await
    }
    async fn trace(&self, outpoint: &str) -> Result<alkanes_cli_common::proto::alkanes::Trace> {
        self.provider.trace(outpoint).await
    }
    async fn get_block(&self, height: u64) -> Result<alkanes_cli_common::proto::alkanes::BlockResponse> {
        <ConcreteProvider as AlkanesProvider>::get_block(&self.provider, height).await
    }
    async fn sequence(&self) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.sequence().await
    }
    async fn spendables_by_address(&self, address: &str) -> Result<alkanes_cli_common::JsonValue> {
        self.provider.spendables_by_address(address).await
    }
    async fn trace_block(&self, height: u64) -> Result<alkanes_cli_common::proto::alkanes::Trace> {
        self.provider.trace_block(height).await
    }
    async fn get_bytecode(&self, alkane_id: &str, block_tag: Option<String>) -> Result<String> {
        <ConcreteProvider as AlkanesProvider>::get_bytecode(&self.provider, alkane_id, block_tag).await
    }
    async fn inspect(&self, target: &str, config: alkanes_cli_common::alkanes::AlkanesInspectConfig) -> Result<alkanes_cli_common::alkanes::AlkanesInspectResult> {
        self.provider.inspect(target, config).await
    }
    async fn get_balance(&self, address: Option<&str>) -> Result<Vec<alkanes_cli_common::alkanes::AlkaneBalance>> {
        <ConcreteProvider as AlkanesProvider>::get_balance(&self.provider, address).await
    }
}

#[async_trait(?Send)]
impl MonitorProvider for SystemAlkanes {
    async fn monitor_blocks(&self, start: Option<u64>) -> Result<()> {
        self.provider.monitor_blocks(start).await
    }
    async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
        self.provider.get_block_events(height).await
    }
}

#[async_trait(?Send)]
impl KeystoreProvider for SystemAlkanes {
    async fn derive_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams, script_types: &[&str], start_index: u32, count: u32) -> Result<Vec<KeystoreAddress>> {
        self.provider.derive_addresses(master_public_key, network_params, script_types, start_index, count).await
    }
    async fn get_default_addresses(&self, master_public_key: &str, network_params: &alkanes_cli_common::network::NetworkParams) -> Result<Vec<KeystoreAddress>> {
        self.provider.get_default_addresses(master_public_key, network_params).await
    }
    fn parse_address_range(&self, range_spec: &str) -> Result<(String, u32, u32)> {
        self.provider.parse_address_range(range_spec)
    }
    async fn get_keystore_info(&self, master_fingerprint: &str, created_at: u64, version: &str) -> Result<KeystoreInfo> {
        self.provider.get_keystore_info(master_fingerprint, created_at, version).await
    }
    async fn get_address(&self, address_type: &str, index: u32) -> Result<String> {
        <ConcreteProvider as KeystoreProvider>::get_address(&self.provider, address_type, index).await
    }
    async fn derive_address_from_path(&self, _master_public_key: &str, _path: &bitcoin::bip32::DerivationPath, _script_type: &str, _network_params: &alkanes_cli_common::network::NetworkParams) -> Result<KeystoreAddress> {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl OrdProvider for SystemAlkanes {
    async fn get_inscription(&self, inscription_id: &str) -> Result<alkanes_cli_common::ord::Inscription> {
        self.provider.get_inscription(inscription_id).await
    }
    async fn get_inscriptions_in_block(&self, block_hash: &str) -> Result<alkanes_cli_common::ord::Inscriptions> {
        self.provider.get_inscriptions_in_block(block_hash).await
    }
    async fn get_ord_address_info(&self, address: &str) -> Result<alkanes_cli_common::ord::AddressInfo> {
        self.provider.get_ord_address_info(address).await
    }
    async fn get_block_info(&self, query: &str) -> Result<alkanes_cli_common::ord::Block> {
        self.provider.get_block_info(query).await
    }
    async fn get_ord_block_count(&self) -> Result<u64> {
        self.provider.get_ord_block_count().await
    }
    async fn get_ord_blocks(&self) -> Result<alkanes_cli_common::ord::Blocks> {
        self.provider.get_ord_blocks().await
    }
    async fn get_children(&self, inscription_id: &str, page: Option<u32>) -> Result<alkanes_cli_common::ord::Children> {
        self.provider.get_children(inscription_id, page).await
    }
    async fn get_content(&self, inscription_id: &str) -> Result<Vec<u8>> {
        self.provider.get_content(inscription_id).await
    }
    async fn get_inscriptions(&self, page: Option<u32>) -> Result<alkanes_cli_common::ord::Inscriptions> {
        self.provider.get_inscriptions(page).await
    }
    async fn get_output(&self, output: &str) -> Result<alkanes_cli_common::ord::Output> {
        self.provider.get_output(output).await
    }
    async fn get_parents(&self, inscription_id: &str, page: Option<u32>) -> Result<alkanes_cli_common::ord::ParentInscriptions> {
        self.provider.get_parents(inscription_id, page).await
    }
    async fn get_rune(&self, rune: &str) -> Result<alkanes_cli_common::ord::RuneInfo> {
        self.provider.get_rune(rune).await
    }
    async fn get_runes(&self, page: Option<u32>) -> Result<alkanes_cli_common::ord::Runes> {
        self.provider.get_runes(page).await
    }
    async fn get_sat(&self, sat: u64) -> Result<alkanes_cli_common::ord::SatResponse> {
        self.provider.get_sat(sat).await
    }
    async fn get_tx_info(&self, txid: &str) -> Result<alkanes_cli_common::ord::TxInfo> {
        self.provider.get_tx_info(txid).await
    }
}

#[async_trait(?Send)]
impl UtxoProvider for SystemAlkanes {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<Utxo>> {
        self.provider.get_utxos_by_spec(spec).await
    }
}

impl Clone for SystemAlkanes {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            keystore_manager: self.keystore_manager.clone(),
            args: self.args.clone(),
        }
    }
}

// Implement the individual system traits
#[async_trait(?Send)]
impl SystemWallet for SystemAlkanes {
   async fn execute_wallet_command(&self, command: WalletCommands) -> alkanes_cli_common::Result<()> {
        let mut provider = self.provider.clone(); // Clone to allow mutation for unlocking

        // Conditionally load wallet based on command requirements
        if command.requires_signing() {
            // For signing commands, ensure the full wallet is loaded, prompting for passphrase if needed
            if let alkanes_cli_common::provider::WalletState::Locked(_) = provider.get_wallet_state() {
                let passphrase = if let Some(ref pass) = self.args.passphrase {
                    pass.clone()
                } else {
                    rpassword::prompt_password("Enter passphrase to unlock keystore for signing: ")
                        .map_err(|e| AlkanesError::Wallet(format!("Failed to get passphrase: {e}")))?
                };
                provider.unlock_wallet(&passphrase).await?;
            } else if let alkanes_cli_common::provider::WalletState::None = provider.get_wallet_state() {
                 return Err(AlkanesError::Wallet("No wallet found. Please create or specify a wallet file.".to_string()));
            }
        }

       let res: anyhow::Result<()> = match command {
           WalletCommands::Create { mnemonic } => {
               println!("🔐 Creating encrypted keystore...");

               let final_passphrase = if let Some(pass) = self.args.passphrase.clone() {
                   pass
               } else {
                   let pass = rpassword::prompt_password("Enter passphrase: ")?;
                   let confirmation = rpassword::prompt_password("Confirm passphrase: ")?;
                   if pass != confirmation {
                       return Err(AlkanesError::Wallet("Passphrases do not match".to_string()));
                   }
                   pass
               };

               // Create keystore parameters, including the passphrase from CLI args
               let keystore_params = KeystoreCreateParams {
                   mnemonic: mnemonic.clone(),
                   passphrase: Some(final_passphrase),
                   network: provider.get_network(),
                   address_count: 5, // This parameter is now unused but kept for compatibility
                   hd_path: None,
               };

               // Create the keystore
               let (keystore, mnemonic_phrase) = self.keystore_manager.create_keystore(keystore_params).await?;
               
               // FIXED: Use the wallet file path from provider (which respects --wallet-file argument)
               #[cfg(not(target_arch = "wasm32"))]
               let wallet_file = provider.get_wallet_path()
                   .ok_or_else(|| anyhow!("No wallet file path configured"))?
                   .to_string_lossy()
                   .to_string();
               
               #[cfg(target_arch = "wasm32")]
               let wallet_file = provider.get_wallet_path()
                   .ok_or_else(|| anyhow!("No wallet file path configured"))?
                   .clone();
               
               // Save keystore to file
               self.keystore_manager.save_keystore(&keystore, &wallet_file).await?;
                
                // Get first P2WPKH address for display using dynamic derivation
                let default_addresses = KeystoreManager::get_default_addresses(&self.keystore_manager, &keystore, provider.get_network())?;
                let first_p2wpkh = default_addresses.iter()
                    .find(|addr| addr.script_type == "p2wpkh" && addr.index == 0)
                    .map(|addr| addr.address.clone())
                    .unwrap_or_else(|| "No P2WPKH address generated".to_string());
                
                // Get network name for display
                let network_name = match provider.get_network() {
                    bitcoin::Network::Bitcoin => "mainnet",
                    bitcoin::Network::Testnet => "testnet",
                    bitcoin::Network::Signet => "signet",
                    bitcoin::Network::Regtest => "regtest",
                    _ => "custom",
                };
                
                println!("✅ Wallet keystore created successfully!");
                println!("📁 Keystore saved to: {wallet_file}");
                println!("🔑 Mnemonic: {mnemonic_phrase}");
                println!("⚠️  IMPORTANT: Save this mnemonic phrase in a secure location!");
                println!("🏠 First {network_name} P2WPKH address: {first_p2wpkh}");
                println!("🔐 Keystore is encrypted and armored");
                
                // Show keystore info
                let info = self.keystore_manager.get_keystore_info(&keystore);
                println!("🔍 Master Fingerprint: {}", info.master_fingerprint);
                println!("📅 Created: {}", info.created_at);
                println!("🏷️  Version: {}", info.version);
                
                println!("
💡 Use 'deezel wallet addresses' to see all address types");
                println!("💡 Use 'deezel wallet addresses p2tr:0-10' for specific ranges");
                
                Ok(())
            },
           WalletCommands::Restore { mnemonic } => {
                println!("🔐 Restoring wallet from mnemonic...");

                // Create keystore parameters, including the passphrase from CLI args
                let keystore_params = KeystoreCreateParams {
                    mnemonic: Some(mnemonic),
                    passphrase: self.args.passphrase.clone(),
                    network: provider.get_network(),
                    address_count: 5, // This parameter is now unused but kept for compatibility
                    hd_path: None,
                };

                // Create the keystore
                let (keystore, mnemonic_phrase) = self.keystore_manager.create_keystore(keystore_params).await?;
                
                // Use the wallet file path from provider (which respects --wallet-file argument)
                #[cfg(not(target_arch = "wasm32"))]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .to_string_lossy()
                    .to_string();
                
                #[cfg(target_arch = "wasm32")]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .clone();
                
                // Save keystore to file
                self.keystore_manager.save_keystore(&keystore, &wallet_file).await?;
                
                // Get first P2WPKH address for display using dynamic derivation
                let default_addresses = KeystoreManager::get_default_addresses(&self.keystore_manager, &keystore, provider.get_network())?;
                let first_p2wpkh = default_addresses.iter()
                    .find(|addr| addr.script_type == "p2wpkh" && addr.index == 0)
                    .map(|addr| addr.address.clone())
                    .unwrap_or_else(|| "No P2WPKH address generated".to_string());
                
                // Get network name for display
                let network_name = match provider.get_network() {
                    bitcoin::Network::Bitcoin => "mainnet",
                    bitcoin::Network::Testnet => "testnet",
                    bitcoin::Network::Signet => "signet",
                    bitcoin::Network::Regtest => "regtest",
                    _ => "custom",
                };
                
                println!("✅ Wallet keystore restored successfully!");
                println!("📁 Keystore saved to: {wallet_file}");
                println!("🔑 Mnemonic: {mnemonic_phrase}");
                println!("⚠️  IMPORTANT: Save this mnemonic phrase in a secure location!");
                println!("🏠 First {network_name} P2WPKH address: {first_p2wpkh}");
                println!("🔐 Keystore is encrypted and armored");
                
                // Show keystore info
                let info = self.keystore_manager.get_keystore_info(&keystore);
                println!("🔍 Master Fingerprint: {}", info.master_fingerprint);
                println!("📅 Created: {}", info.created_at);
                println!("🏷️  Version: {}", info.version);
                
                println!("
💡 Use 'deezel wallet addresses' to see all address types");
                println!("💡 Use 'deezel wallet addresses p2tr:0-10' for specific ranges");
                
                Ok(())
            },
           WalletCommands::Info => {
                // Use the wallet file path from provider
                #[cfg(not(target_arch = "wasm32"))]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .to_string_lossy()
                    .to_string();
                
                #[cfg(target_arch = "wasm32")]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .clone();

                #[cfg(not(target_arch = "wasm32"))]
                if !std::path::Path::new(&wallet_file).exists() {
                    println!("❌ No keystore found. Please create a wallet first using 'deezel wallet create'");
                    return Ok(());
                }

                // Load keystore metadata without requiring passphrase
                let keystore_metadata = self.keystore_manager.load_keystore_metadata_from_file(&wallet_file).await?;
                let info = self.keystore_manager.get_keystore_info(&keystore_metadata);
                let network = provider.get_network();

                println!("💼 Wallet Information (Locked)");
                println!("═════════════════════════════");
                println!("🔍 Master Fingerprint: {}", info.master_fingerprint);
                println!("📅 Created: {}", chrono::DateTime::from_timestamp(info.created_at as i64, 0).map(|dt| dt.to_rfc2822()).unwrap_or_else(|| "Invalid date".to_string()));
                println!("🏷️  Version: {}", info.version);
                println!("🌐 Network: {network:?}");

                // Display first 5 addresses of each type
                println!("
📋 Default Addresses (derived from public key):");
                let default_addresses = self.keystore_manager.get_default_addresses_from_metadata(&keystore_metadata, network, None)?;
                
                let mut grouped_addresses: std::collections::HashMap<String, Vec<&alkanes_cli_common::traits::KeystoreAddress>> = std::collections::HashMap::new();
                for addr in &default_addresses {
                    grouped_addresses.entry(addr.script_type.clone()).or_default().push(addr);
                }

                for (script_type, addrs) in grouped_addresses {
                    println!("
  {}:", script_type.to_uppercase());
                    for addr in addrs {
                        println!("    {}. {} (index: {})", addr.index, addr.address, addr.index);
                    }
                }

                println!("
💡 To see balances or send transactions, unlock the wallet by providing the --passphrase argument or by running a command that requires signing (e.g., 'wallet send').");

                Ok(())
            },
           WalletCommands::Balance { raw, addresses } => {
                let address_list = if let Some(addr_str) = addresses {
                    Some(resolve_addresses(&addr_str, &provider).await?)
                } else {
                    None
                };

               let balance = WalletProvider::get_balance(&provider, address_list).await?;
               
               if raw {
                   println!("{}", serde_json::to_string_pretty(&balance)?);
               } else {
                   println!("💰 Wallet Balance");
                   println!("═══════════════");
                   println!("✅ Confirmed: {} sats", balance.confirmed);
                   println!("⏳ Pending:   {} sats", balance.pending);
                   println!("📊 Total:     {} sats", (balance.confirmed as i64 + balance.pending));
               }
               Ok(())
           },
           WalletCommands::Addresses { ranges, hd_path, network, all_networks, magic, raw } => {
                // FIXED: Use the wallet file path from provider (which respects --wallet-file argument)
                #[cfg(not(target_arch = "wasm32"))]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .to_string_lossy()
                    .to_string();
                
                #[cfg(target_arch = "wasm32")]
                let wallet_file = provider.get_wallet_path()
                    .ok_or_else(|| anyhow!("No wallet file path configured"))?
                    .clone();
                
                // Check if keystore exists
                #[cfg(not(target_arch = "wasm32"))]
                if !std::path::Path::new(&wallet_file).exists() {
                    println!("❌ No keystore found. Please create a wallet first using 'deezel wallet create'");
                    return Ok(());
                }
                
                // ENHANCED: Load keystore metadata without requiring passphrase (addresses command only needs master public key)
                let keystore_metadata = self.keystore_manager.load_keystore_metadata_from_file(&wallet_file).await?;
                
                // Determine which networks to show addresses for
                let networks_to_show = if all_networks {
                    // Show addresses for all supported networks
                    vec![
                        bitcoin::Network::Bitcoin,
                        bitcoin::Network::Testnet,
                        bitcoin::Network::Signet,
                        bitcoin::Network::Regtest,
                    ]
                } else if let Some(ref network_name) = network {
                    // Show addresses for specific network
                    match alkanes_cli_common::network::NetworkParams::from_network_str(network_name) {
                        Ok(params) => vec![params.network],
                        Err(e) => {
                            println!("❌ Invalid network '{network_name}': {e}");
                            println!("💡 Supported networks: {}", alkanes_cli_common::network::NetworkParams::supported_networks().join(", "));
                            return Ok(());
                        }
                    }
                } else {
                    // Default: show addresses for current provider network
                    vec![provider.get_network()]
                };
                
                // Handle custom magic bytes if provided, OR use global magic bytes from args
                let custom_network_params = if let Some(ref magic_str) = magic {
                    // Local --magic flag takes precedence
                    match alkanes_cli_common::network::NetworkParams::from_magic_str(magic_str) {
                        Ok((p2pkh_prefix, p2sh_prefix, bech32_hrp)) => {
                            Some(alkanes_cli_common::network::NetworkParams::with_custom_magic(
                                provider.get_network(),
                                p2pkh_prefix,
                                p2sh_prefix,
                                bech32_hrp,
                            ))
                        },
                        Err(e) => {
                            println!("❌ Invalid magic bytes format: {e}");
                            return Ok(());
                        }
                    }
                } else if let Some(ref global_magic_str) = self.args.magic {
                    // Use global -p flag magic bytes if no local --magic specified
                    match alkanes_cli_common::network::NetworkParams::from_magic_str(global_magic_str) {
                        Ok((p2pkh_prefix, p2sh_prefix, bech32_hrp)) => {
                            Some(alkanes_cli_common::network::NetworkParams::with_custom_magic(
                                provider.get_network(),
                                p2pkh_prefix,
                                p2sh_prefix,
                                bech32_hrp,
                            ))
                        },
                        Err(_) => {
                            // If global magic parsing fails, try to get network params from provider string
                            alkanes_cli_common::network::NetworkParams::from_network_str(&self.args.rpc_config.provider).ok()
                        }
                    }
                } else if self.args.rpc_config.provider != "regtest" {
                    // Use network params from provider if it's not the default regtest
                    alkanes_cli_common::network::NetworkParams::from_network_str(&self.args.rpc_config.provider).ok()
                } else {
                    None
                };
                
                let mut all_addresses = Vec::new();
                
                for network in networks_to_show {
                    let network_name = match network {
                        bitcoin::Network::Bitcoin => "mainnet",
                        bitcoin::Network::Testnet => "testnet",
                        bitcoin::Network::Signet => "signet",
                        bitcoin::Network::Regtest => "regtest",
                        _ => "custom",
                    };
                    
                    let addresses = if let Some(range_specs) = &ranges {
                        // Parse and derive addresses for specified ranges
                        let mut network_addresses = Vec::new();
                        
                        for range_spec in range_specs {
                            let (script_type, start_index, count) = KeystoreManager::parse_address_range(&self.keystore_manager, range_spec)?;
                            let script_types = [script_type.as_str()];
                            let derived = KeystoreManager::derive_addresses_from_metadata(&self.keystore_manager, &keystore_metadata, network, &script_types, start_index, count, custom_network_params.as_ref())?;
                            network_addresses.extend(derived);
                        }
                        
                        network_addresses
                    } else {
                        // Default behavior: show first 5 addresses of each type for current network
                        KeystoreManager::get_default_addresses_from_metadata(&self.keystore_manager, &keystore_metadata, network, custom_network_params.as_ref())?
                    };
                    
                    // Add network information to each address
                    for mut addr in addresses {
                        addr.network = Some(network_name.to_string());
                        all_addresses.push(addr);
                    }
                }
                
                if raw {
                    // Convert to serializable format
                    let serializable_addresses: Vec<serde_json::Value> = all_addresses.iter().map(|addr| {
                        serde_json::json!({
                            "address": addr.address,
                            "script_type": addr.script_type,
                            "derivation_path": addr.derivation_path,
                            "index": addr.index,
                            "network": addr.network
                        })
                    }).collect();
                    println!("{}", serde_json::to_string_pretty(&serializable_addresses)?);
                } else {
                    if all_networks {
                        println!("🏠 Wallet Addresses (All Networks)");
                    } else if let Some(network_name) = &network {
                        println!("🏠 Wallet Addresses ({network_name})");
                    } else {
                        let current_network_name = match provider.get_network() {
                            bitcoin::Network::Bitcoin => "mainnet",
                            bitcoin::Network::Testnet => "testnet",
                            bitcoin::Network::Signet => "signet",
                            bitcoin::Network::Regtest => "regtest",
                            _ => "custom",
                        };
                        println!("🏠 Wallet Addresses ({current_network_name})");
                    }
                    println!("═════════════════════════════");
                    
                    // Display network magic bytes when a specific network is selected
                    if let Some(ref network_name) = network {
                        if let Ok(network_params) = alkanes_cli_common::network::NetworkParams::from_network_str(network_name) {
                            println!("🔮 Network Magic Bytes:");
                            println!("   Bech32 HRP: {}", network_params.bech32_prefix);
                            println!("   P2PKH Prefix: 0x{:02x}", network_params.p2pkh_prefix);
                            println!("   P2SH Prefix: 0x{:02x}", network_params.p2sh_prefix);
                            println!("   Format: {}:{:02x}:{:02x}", network_params.bech32_prefix, network_params.p2pkh_prefix, network_params.p2sh_prefix);
                            println!();
                        }
                    }
                    
                    if let Some(ref hd_path_custom) = hd_path {
                        println!("🛤️  Custom HD Path: {hd_path_custom}");
                        println!();
                    }
                    
                    if let Some(ref magic_str) = magic {
                        println!("🔮 Custom Magic Bytes: {magic_str}");
                        println!();
                    }
                    
                    // Group addresses by network and script type for better display
                    let mut grouped_addresses: std::collections::HashMap<String, std::collections::HashMap<String, Vec<&alkanes_cli_common::traits::KeystoreAddress>>> = std::collections::HashMap::new();
                    for addr in &all_addresses {
                        let network_key = addr.network.as_ref().unwrap_or(&"unknown".to_string()).clone();
                        grouped_addresses.entry(network_key).or_default()
                            .entry(addr.script_type.clone()).or_default().push(addr);
                    }
                    
                    for (network_name, script_types) in grouped_addresses {
                        if all_networks {
                            println!("🌐 Network: {}", network_name.to_uppercase());
                            println!("─────────────────────");
                        }
                        
                        for (script_type, addrs) in script_types {
                            println!("📋 {} Addresses:", script_type.to_uppercase());
                            for addr in addrs {
                                println!("  {}. {} (index: {})", addr.index, addr.address, addr.index);
                                println!("     Path: {}", addr.derivation_path);
                            }
                            println!();
                        }
                        
                        if all_networks {
                            println!();
                        }
                    }
                }
                Ok(())
            },
           WalletCommands::Send { address, amount, fee_rate, send_all, from, change, use_rebar, rebar_tier, yes } => {
               // Resolve address identifiers
               let resolved_address = provider.resolve_all_identifiers(&address).await?;
               let resolved_from = if let Some(from_addrs) = from {
                   let mut resolved = Vec::new();
                   for addr in from_addrs {
                       resolved.push(provider.resolve_all_identifiers(&addr).await?);
                   }
                   Some(resolved)
               } else {
                   None
               };
               let resolved_change = if let Some(change_addr) = change {
                   Some(provider.resolve_all_identifiers(&change_addr).await?)
               } else {
                   None
               };
               
               let send_params = SendParams {
                   address: resolved_address,
                   amount,
                   fee_rate,
                   send_all,
                   from: resolved_from,
                   change_address: resolved_change,
                   auto_confirm: yes,
                   use_rebar,
                   rebar_tier,
               };
               
               match provider.send(send_params).await {
                   Ok(txid) => {
                       println!("✅ Transaction sent successfully!");
                       println!("🔗 Transaction ID: {txid}");
                   },
                   Err(e) => {
                       println!("❌ Failed to send transaction: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::SendAll { address, fee_rate, yes } => {
               // Resolve address identifiers
               let resolved_address = provider.resolve_all_identifiers(&address).await?;
               
               let send_params = SendParams {
                   address: resolved_address,
                   amount: 0, // Will be ignored since send_all is true
                   fee_rate,
                   send_all: true,
                   from: None,
                   change_address: None,
                   auto_confirm: yes,
                   use_rebar: false,
                   rebar_tier: 1,
               };
               
               match provider.send(send_params).await {
                   Ok(txid) => {
                       println!("✅ All funds sent successfully!");
                       println!("🔗 Transaction ID: {txid}");
                   },
                   Err(e) => {
                       println!("❌ Failed to send all funds: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::CreateTx { address, amount, fee_rate, send_all, yes } => {
               // Resolve address identifiers
               let resolved_address = provider.resolve_all_identifiers(&address).await?;
               
               let create_params = SendParams {
                   address: resolved_address,
                   amount,
                   fee_rate,
                   send_all,
                   from: None,
                   change_address: None,
                   auto_confirm: yes,
                   use_rebar: false,
                   rebar_tier: 1,
               };
               
               match provider.create_transaction(create_params).await {
                   Ok(tx_hex) => {
                       println!("✅ Transaction created successfully!");
                       println!("📄 Transaction hex: {tx_hex}");
                   },
                   Err(e) => {
                       println!("❌ Failed to create transaction: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::SignTx { tx_hex, from_file, truncate_excess_vsize } => {
               // Read hex from file or argument
               let hex_string = if let Some(file_path) = from_file {
                   std::fs::read_to_string(file_path)
                       .map_err(|e| AlkanesError::Storage(format!("Failed to read file: {}", e)))?
                       .trim()
                       .to_string()
               } else {
                   tx_hex.ok_or_else(|| AlkanesError::InvalidParameters("No transaction hex or file provided".to_string()))?
               };
               
               // Decode the unsigned transaction first
               use bitcoin::consensus::Decodable;
               let tx_bytes = hex::decode(&hex_string).map_err(|e| AlkanesError::InvalidParameters(format!("Invalid hex: {}", e)))?;
               let unsigned_tx = bitcoin::Transaction::consensus_decode(&mut &tx_bytes[..])
                   .map_err(|e| AlkanesError::InvalidParameters(format!("Invalid transaction: {}", e)))?;
               
               const MAX_TX_SIZE: usize = 1_000_000; // Bitcoin consensus limit (1 MB)
               const SAFETY_MARGIN: f64 = 0.98; // 2% safety margin
               
               let mut working_tx = unsigned_tx.clone();
               let original_input_count = working_tx.input.len();
               
               // If truncate flag is set, estimate signed size and truncate if needed
               if truncate_excess_vsize {
                   // Detect fee rate from unsigned transaction by calculating:
                   // original_fee_rate = (total_input - output) / unsigned_vsize
                   // This works because for --send-all: output = input - fee
                   let unsigned_vsize = hex_string.len() / 2; // Approximate unsigned size
                   let original_output_amount = working_tx.output.get(0)
                       .map(|o| o.value.to_sat())
                       .unwrap_or(0);
                   
                   // Calculate total input (assume 1999 sats per UTXO - P2WPKH standard)
                   let sats_per_input = 1999u64;
                   let total_input = working_tx.input.len() as u64 * sats_per_input;
                   let unsigned_fee = total_input.saturating_sub(original_output_amount);
                   
                   // Detect fee rate from unsigned transaction
                   let detected_fee_rate = if unsigned_vsize > 0 {
                       unsigned_fee as f64 / unsigned_vsize as f64
                   } else {
                       2.1 // Default fallback
                   };
                   
                   eprintln!("📊 Detected fee rate from unsigned tx: {:.4} sat/vB", detected_fee_rate);
                   
                   // Estimate signed size: each input adds ~66 bytes of witness data
                   let estimated_signed_size = unsigned_vsize + (working_tx.input.len() * 66);
                   
                   if estimated_signed_size > MAX_TX_SIZE {
                       let target_size = (MAX_TX_SIZE as f64 * SAFETY_MARGIN) as usize;
                       
                       // Calculate max inputs: size = overhead + (inputs * 107 bytes)
                       let overhead = 53; // Base tx + 1 P2TR output
                       let bytes_per_signed_input = 107;
                       let max_inputs = (target_size - overhead) / bytes_per_signed_input;
                       
                       if max_inputs < working_tx.input.len() {
                           eprintln!("⚠️  Transaction will exceed consensus limit ({} MB)", MAX_TX_SIZE / 1_000_000);
                           eprintln!("⚠️  Truncating inputs: {} → {} inputs", original_input_count, max_inputs);
                           eprintln!("⚠️  Removed {} inputs", original_input_count - max_inputs);
                           
                           working_tx.input.truncate(max_inputs);
                           
                           // Recalculate output amount with detected fee rate
                           let truncated_input = max_inputs as u64 * sats_per_input;
                           
                           // Calculate proper vSize for signed P2TR transaction
                           // P2TR signed input: 229 WU / 4 = 57.25 vbytes (use 58)
                           let base_vsize = 10u64;
                           let input_vsize = 58u64; // P2TR with witness discount
                           let output_vsize = 43u64;
                           let witness_overhead = 1u64;
                           let truncated_tx_vsize = base_vsize + 
                               (max_inputs as u64 * input_vsize) + 
                               (working_tx.output.len() as u64 * output_vsize) +
                               witness_overhead;
                           
                           let truncated_fee = (truncated_tx_vsize as f64 * detected_fee_rate).ceil() as u64;
                           let truncated_output = truncated_input.saturating_sub(truncated_fee);
                           
                           if let Some(output) = working_tx.output.get_mut(0) {
                               output.value = bitcoin::Amount::from_sat(truncated_output);
                           }
                           
                           let actual_fee_rate = truncated_fee as f64 / truncated_tx_vsize as f64;
                           
                           eprintln!("📊 Adjusted transaction:");
                           eprintln!("   Inputs: {} UTXOs", max_inputs);
                           eprintln!("   Input amount: {} sats", truncated_input);
                           eprintln!("   Transaction vSize: {} vbytes", truncated_tx_vsize);
                           eprintln!("   Fee: {} sats", truncated_fee);
                           eprintln!("   Output amount: {} sats", truncated_output);
                           eprintln!("   Fee rate: {:.4} sat/vB", actual_fee_rate);
                           eprintln!("");
                       }
                   }
               }
               
               // Serialize the (possibly truncated) transaction
               use bitcoin::consensus::Encodable;
               let mut truncated_bytes = Vec::new();
               working_tx.consensus_encode(&mut truncated_bytes)
                   .map_err(|e| AlkanesError::InvalidParameters(format!("Failed to encode transaction: {}", e)))?;
               let truncated_hex = hex::encode(truncated_bytes);
               
               match provider.sign_transaction(truncated_hex).await {
                   Ok(signed_hex) => {
                       let signed_bytes = hex::decode(&signed_hex).map_err(|e| AlkanesError::InvalidParameters(format!("Invalid signed hex: {}", e)))?;
                       let signed_size = signed_bytes.len();
                       
                       println!("✅ Transaction signed successfully!");
                       
                       if truncate_excess_vsize && original_input_count != working_tx.input.len() {
                           println!("⚠️  Transaction was truncated to fit consensus limit");
                           println!("   Original inputs: {}", original_input_count);
                           println!("   Final inputs: {}", working_tx.input.len());
                           println!("   Removed inputs: {}", original_input_count - working_tx.input.len());
                       }
                       
                       println!("📏 Signed transaction size: {} bytes ({:.2} KB)", signed_size, signed_size as f64 / 1024.0);
                       
                       if signed_size > MAX_TX_SIZE {
                           eprintln!("❌ WARNING: Signed transaction still exceeds consensus limit!");
                           eprintln!("   Size: {} bytes", signed_size);
                           eprintln!("   Limit: {} bytes", MAX_TX_SIZE);
                           eprintln!("   You may need to reduce inputs further");
                       } else {
                           println!("✅ Transaction size is within consensus limit");
                       }
                       
                       println!("📄 Signed transaction hex:");
                       println!("{signed_hex}");
                   },
                   Err(e) => {
                       println!("❌ Failed to sign transaction: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::Sign { tx_hex, from_file } => {
               // This command uses --wallet-key-file for signing
               // Read hex from file or argument
               let hex_string = if let Some(file_path) = from_file {
                   std::fs::read_to_string(file_path)
                       .map_err(|e| AlkanesError::Storage(format!("Failed to read file: {}", e)))?
                       .trim()
                       .to_string()
               } else {
                   tx_hex.ok_or_else(|| AlkanesError::InvalidParameters("No transaction hex or file provided".to_string()))?
               };
               
               match provider.sign_transaction(hex_string).await {
                   Ok(signed_hex) => {
                       println!("✅ Transaction signed successfully!");
                       println!("📄 Signed transaction hex:");
                       println!("{signed_hex}");
                   },
                   Err(e) => {
                       eprintln!("❌ Failed to sign transaction: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::DecodeTx { tx_hex, file, raw } => {
               use bitcoin::consensus::deserialize;
               use bitcoin::Transaction;
               
               // Read hex from file or argument
               let hex_string = if let Some(file_path) = file {
                   std::fs::read_to_string(file_path)
                       .map_err(|e| AlkanesError::Storage(format!("Failed to read file: {}", e)))?
                       .trim()
                       .to_string()
               } else {
                   tx_hex.ok_or_else(|| AlkanesError::InvalidParameters("No transaction hex or file provided".to_string()))?
               };
               
               let tx_bytes = hex::decode(&hex_string)
                   .map_err(|e| AlkanesError::Parse(format!("Invalid hex: {}", e)))?;
               
               let tx: Transaction = deserialize(&tx_bytes)
                   .map_err(|e| AlkanesError::Parse(format!("Invalid transaction: {}", e)))?;
               
               if raw {
                   // Output as JSON
                   let tx_json = serde_json::json!({
                       "txid": tx.compute_txid().to_string(),
                       "version": tx.version.0,
                       "locktime": tx.lock_time.to_consensus_u32(),
                       "size": tx_bytes.len(),
                       "vsize": tx.vsize(),
                       "weight": tx.weight().to_wu(),
                       "vin_count": tx.input.len(),
                       "vout_count": tx.output.len(),
                       "inputs": tx.input.iter().map(|input| {
                           serde_json::json!({
                               "txid": input.previous_output.txid.to_string(),
                               "vout": input.previous_output.vout,
                               "sequence": input.sequence.0,
                               "witness_elements": input.witness.len(),
                           })
                       }).collect::<Vec<_>>(),
                       "outputs": tx.output.iter().map(|output| {
                           serde_json::json!({
                               "value": output.value.to_sat(),
                               "script_pubkey": hex::encode(output.script_pubkey.as_bytes()),
                           })
                       }).collect::<Vec<_>>(),
                   });
                   println!("{}", serde_json::to_string_pretty(&tx_json)?);
               } else {
                   // Pretty print
                   println!("🔍 Transaction Details");
                   println!("═══════════════════════════════════════════════════════════");
                   println!("📋 TXID: {}", tx.compute_txid());
                   println!("📦 Version: {}", tx.version.0);
                   println!("🔒 Locktime: {}", tx.lock_time.to_consensus_u32());
                   println!("");
                   println!("📏 Size Information:");
                   println!("   Raw Size: {} bytes", tx_bytes.len());
                   println!("   Virtual Size: {} vbytes", tx.vsize());
                   println!("   Weight: {} WU", tx.weight().to_wu());
                   println!("");
                   println!("📥 Inputs: {}", tx.input.len());
                   for (i, input) in tx.input.iter().enumerate() {
                       println!("   [{}] {}:{}", i, input.previous_output.txid, input.previous_output.vout);
                       println!("       Sequence: {}", input.sequence.0);
                       println!("       Witness elements: {}", input.witness.len());
                   }
                   println!("");
                   println!("📤 Outputs: {}", tx.output.len());
                   for (i, output) in tx.output.iter().enumerate() {
                       println!("   [{}] {} sats", i, output.value.to_sat());
                       println!("       Script: {}", hex::encode(output.script_pubkey.as_bytes()));
                   }
                   println!("═══════════════════════════════════════════════════════════");
               }
               Ok(())
           },
           WalletCommands::BroadcastTx { tx_hex, yes } => {
               if !yes {
                   println!("⚠️  About to broadcast transaction: {tx_hex}");
                   println!("Do you want to continue? (y/N)");
                   
                   let mut input = String::new();
                   std::io::stdin().read_line(&mut input)?;
                   
                   if !input.trim().to_lowercase().starts_with('y') {
                       println!("❌ Transaction broadcast cancelled");
                       return Ok(());
                   }
               }
               
               match provider.broadcast(&tx_hex).await {
                   Ok(txid) => {
                       println!("✅ Transaction broadcast successfully!");
                       println!("🔗 Transaction ID: {txid}");
                   },
                   Err(e) => {
                       println!("❌ Failed to broadcast transaction: {e}");
                       return Err(e);
                   }
               }
               Ok(())
           },
           WalletCommands::Utxos { raw, include_frozen, addresses } => {
               let address_list = if let Some(addr_str) = addresses {
                   let resolved_addresses = provider.resolve_all_identifiers(&addr_str).await?;
                   Some(resolved_addresses.split(',').map(|s| s.trim().to_string()).collect())
               } else {
                   None
               };
               
               let utxos = provider.get_utxos(include_frozen, address_list).await?;
               
               if raw {
                   // Convert to serializable format
                   let serializable_utxos: Vec<serde_json::Value> = utxos.iter().map(|(_outpoint, utxo_info)| {
                       serde_json::json!({
                           "txid": utxo_info.txid,
                           "vout": utxo_info.vout,
                           "amount": utxo_info.amount,
                           "address": utxo_info.address,
                           "confirmations": utxo_info.confirmations,
                           "frozen": utxo_info.frozen,
                           "freeze_reason": utxo_info.freeze_reason,
                           "block_height": utxo_info.block_height,
                           "has_inscriptions": utxo_info.has_inscriptions,
                           "has_runes": utxo_info.has_runes,
                           "has_alkanes": utxo_info.has_alkanes,
                           "is_coinbase": utxo_info.is_coinbase
                       })
                   }).collect();
                   println!("{}", serde_json::to_string_pretty(&serializable_utxos)?);
               } else {
                   println!("💰 Wallet UTXOs");
                   println!("═══════════════");
                   
                   if utxos.is_empty() {
                       println!("No UTXOs found");
                   } else {
                       let total_amount: u64 = utxos.iter().map(|(_, u)| u.amount).sum();
                       println!("📊 Total: {} UTXOs, {} sats
", utxos.len(), total_amount);
                       
                       for (i, (outpoint, utxo_info)) in utxos.iter().enumerate() {
                           println!("{}. 🔗 {}:{}", i + 1, outpoint.txid, outpoint.vout);
                           println!("   💰 Amount: {} sats", utxo_info.amount);
                           println!("   🏠 Address: {}", utxo_info.address);
                           println!("   ✅ Confirmations: {}", utxo_info.confirmations);
                           
                           if let Some(block_height) = utxo_info.block_height {
                               println!("   📦 Block: {block_height}");
                           }
                           
                           // Show special properties
                           let mut properties = Vec::new();
                           if utxo_info.is_coinbase {
                               properties.push("coinbase");
                           }
                           if utxo_info.has_inscriptions {
                               properties.push("inscriptions");
                           }
                           if utxo_info.has_runes {
                               properties.push("runes");
                           }
                           if utxo_info.has_alkanes {
                               properties.push("alkanes");
                           }
                           if !properties.is_empty() {
                               println!("   🏷️  Properties: {}", properties.join(", "));
                           }
                           
                           if utxo_info.frozen {
                               println!("   ❄️  Status: FROZEN");
                               if let Some(reason) = &utxo_info.freeze_reason {
                                   println!("   📝 Reason: {reason}");
                               }
                           } else {
                               println!("   ✅ Status: spendable");
                           }
                           
                           if i < utxos.len() - 1 {
                               println!();
                           }
                       }
                   }
               }
               Ok(())
           },
           WalletCommands::FreezeUtxo { utxo, reason } => {
               provider.freeze_utxo(utxo.clone(), reason).await?;
               println!("❄️  UTXO {utxo} frozen successfully");
               Ok(())
           },
           WalletCommands::UnfreezeUtxo { utxo } => {
               provider.unfreeze_utxo(utxo.clone()).await?;
               println!("✅ UTXO {utxo} unfrozen successfully");
               Ok(())
           },
           WalletCommands::History { count, raw, address } => {
               let resolved_address = if let Some(addr) = address {
                   Some(provider.resolve_all_identifiers(&addr).await?)
               } else {
                   None
               };
               
               let history = provider.get_history(count, resolved_address).await?;
               
               if raw {
                   // Convert to serializable format
                   let serializable_history: Vec<serde_json::Value> = history.iter().map(|tx| {
                       serde_json::json!({
                           "txid": tx.txid,
                           "block_height": tx.block_height,
                           "block_time": tx.block_time,
                           "confirmed": tx.confirmed,
                           "fee": tx.fee
                       })
                   }).collect();
                   println!("{}", serde_json::to_string_pretty(&serializable_history)?);
               } else {
                   println!("📜 Transaction History");
                   println!("═══════════════════");
                   
                   if history.is_empty() {
                       println!("No transactions found");
                   } else {
                       for (i, tx) in history.iter().enumerate() {
                           println!("{}. 🔗 TXID: {}", i + 1, tx.txid);
                           if let Some(fee) = tx.fee {
                               println!("   💰 Fee: {fee} sats");
                           }
                           println!("   ✅ Confirmed: {}", tx.confirmed);
                           
                           if i < history.len() - 1 {
                               println!();
                           }
                       }
                   }
               }
               Ok(())
           },
           WalletCommands::TxDetails { txid, raw } => {
               let details = EsploraProvider::get_tx(&provider, &txid).await?;
               
               if raw {
                   println!("{}", serde_json::to_string_pretty(&details)?);
               } else {
                   println!("📄 Transaction Details");
                   println!("════════════════════");
                   println!("🔗 TXID: {txid}");
                   println!("{}", serde_json::to_string_pretty(&details)?);
               }
               Ok(())
           },
           WalletCommands::EstimateFee { target } => {
               let estimate = provider.estimate_fee(target).await?;
               println!("💰 Fee Estimate");
               println!("═══════════════");
               println!("🎯 Target: {target} blocks");
               println!("💸 Fee rate: {} sat/vB", estimate.fee_rate);
               Ok(())
           },
           WalletCommands::FeeRates => {
               let rates = provider.get_fee_rates().await?;
               println!("💸 Current Fee Rates");
               println!("═══════════════════");
               println!("🚀 Fast: {} sat/vB", rates.fast);
               println!("🚶 Medium: {} sat/vB", rates.medium);
               println!("🐌 Slow: {} sat/vB", rates.slow);
               Ok(())
           },
           WalletCommands::Sync => {
               provider.sync().await?;
               println!("✅ Wallet synchronized with blockchain");
               Ok(())
           },
           WalletCommands::Backup => {
               let backup = provider.backup().await?;
               println!("💾 Wallet Backup");
               println!("═══════════════");
               println!("{backup}");
               Ok(())
           },
           WalletCommands::ListIdentifiers => {
               let identifiers = provider.list_identifiers().await?;
               println!("🏷️  Address Identifiers");
               println!("═════════════════════");
               for identifier in identifiers {
                   println!("  {identifier}");
               }
               Ok(())
           },
       };
       res.map_err(|e| AlkanesError::Wallet(e.to_string()))
   }

    async fn execute_walletinfo_command(&self, raw: bool) -> alkanes_cli_common::Result<()> {
       let provider = &self.provider;
       let address = WalletProvider::get_address(provider).await.map_err(|e| AlkanesError::Wallet(e.to_string()))?;
       let balance = WalletProvider::get_balance(provider, None).await.map_err(|e| AlkanesError::Wallet(e.to_string()))?;
       let network = provider.get_network();
       
       if raw {
           let info = serde_json::json!({
               "address": address,
               "balance": balance.confirmed as i64 + balance.pending,
               "network": format!("{:?}", network),
           });
           println!("{}", serde_json::to_string_pretty(&info).unwrap());
       } else {
           println!("💼 Wallet Information");
           println!("═══════════════════");
           println!("🏠 Address: {address}");
           println!("💰 Balance: {} sats", balance.confirmed as i64 + balance.pending);
           println!("🌐 Network: {network:?}");
       }
       
       Ok(())
   }
}

/// Resolves a comma-separated string of addresses and identifiers into a list of concrete addresses.
async fn resolve_addresses(
    addr_str: &str,
    provider: &ConcreteProvider,
) -> anyhow::Result<Vec<String>> {
    let mut resolved_addresses = Vec::new();
    let keystore = provider.get_keystore()?;

    for part in addr_str.split(',') {
        let trimmed_part = part.trim();
        if trimmed_part.contains(':') && !trimmed_part.starts_with("bc1") && !trimmed_part.starts_with("tb1") && !trimmed_part.starts_with("bcrt1") {
            // It's an identifier, e.g., p2tr:0-10 or p2wpkh:5
            let (script_type, start, count) = KeystoreManager::parse_address_range(&KeystoreManager::new(), trimmed_part)?;
            let derived = KeystoreManager::derive_addresses(&KeystoreManager::new(), keystore, provider.get_network(), &[&script_type], start, count)?;
            resolved_addresses.extend(derived.into_iter().map(|a| a.address));
        } else {
            // It's a concrete address
            resolved_addresses.push(trimmed_part.to_string());
        }
    }
    Ok(resolved_addresses)
}

#[async_trait(?Send)]
impl SystemBitcoind for SystemAlkanes {
   async fn execute_bitcoind_command(&self, command: BitcoindCommands) -> alkanes_cli_common::Result<()> {
       let provider = &self.provider;
       let res: anyhow::Result<()> = match command {
            BitcoindCommands::Getblockcount { raw } => {
                let count = <ConcreteProvider as BitcoinRpcProvider>::get_block_count(provider).await?;
                if raw {
                    println!("{count}");
                } else {
                    println!("{count}");
                }
                Ok(())
            },
            BitcoindCommands::Generatetoaddress { nblocks, address, raw } => {
              // Resolve address identifiers if needed
              let resolved_address = provider.resolve_all_identifiers(&address).await?;
              
              let result = <ConcreteProvider as BitcoinRpcProvider>::generate_to_address(provider, nblocks, &resolved_address).await?;
              if raw {
                  println!("{}", serde_json::to_string_pretty(&result)?);
              } else {
                  println!("Generated {nblocks} blocks to address {resolved_address}");
                  if let Some(block_hashes) = result.as_array() {
                      println!("Block hashes:");
                      for (i, hash) in block_hashes.iter().enumerate() {
                          if let Some(hash_str) = hash.as_str() {
                              println!("  {}: {}", i + 1, hash_str);
                          }
                      }
                  }
              }
              Ok(())
            },
            BitcoindCommands::Getblockchaininfo { raw } => {
                let info = <ConcreteProvider as BitcoinRpcProvider>::get_blockchain_info(provider).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    pretty_print::pretty_print_blockchain_info(&info)?;
                }
                Ok(())
            },
            BitcoindCommands::Getnetworkinfo { raw } => {
                let info = <ConcreteProvider as BitcoinRpcProvider>::get_network_info(provider).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    pretty_print::pretty_print_network_info(&info)?;
                }
                Ok(())
            },
            BitcoindCommands::Getrawtransaction { txid, block_hash, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_raw_transaction(provider, &txid, block_hash.as_deref()).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{result}");
                }
                Ok(())
            },
            BitcoindCommands::Getblock { hash, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_block(provider, &hash, raw).await?;
                if raw {
                    println!("{}", result.as_str().unwrap_or(""));
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Getblockhash { height, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_block_hash(provider, height).await?;
                if raw {
                    println!("{result}");
                } else {
                    println!("{result}");
                }
                Ok(())
            },
            BitcoindCommands::Getblockheader { hash, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_block_header(provider, &hash).await?;
                if raw {
                    println!("{}", result.as_str().unwrap_or(""));
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Getblockstats { hash, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_block_stats(provider, &hash).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Decoderawtransaction { hex, raw } => {
                use bitcoin::consensus::deserialize;
                use bitcoin::Transaction;
                
                let tx_bytes = hex::decode(&hex)?;
                let tx: Transaction = deserialize(&tx_bytes)?;
                
                if raw {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "txid": tx.compute_txid().to_string(),
                        "version": tx.version.0,
                        "locktime": tx.lock_time.to_consensus_u32(),
                        "size": tx_bytes.len(),
                        "vsize": tx.vsize(),
                        "weight": tx.weight().to_wu(),
                        "inputs": tx.input.iter().enumerate().map(|(i, input)| {
                            serde_json::json!({
                                "index": i,
                                "txid": input.previous_output.txid.to_string(),
                                "vout": input.previous_output.vout,
                                "sequence": input.sequence.0,
                                "witness_count": input.witness.len(),
                                "witness_sizes": input.witness.iter().map(|w| w.len()).collect::<Vec<_>>(),
                            })
                        }).collect::<Vec<_>>(),
                        "outputs": tx.output.iter().enumerate().map(|(i, output)| {
                            serde_json::json!({
                                "index": i,
                                "value": output.value.to_sat(),
                                "script_pubkey_hex": hex::encode(output.script_pubkey.as_bytes()),
                                "script_pubkey_asm": format!("{:?}", output.script_pubkey),
                            })
                        }).collect::<Vec<_>>(),
                    }))?);
                } else {
                    println!("📄 Transaction Decoded");
                    println!("═════════════════════");
                    println!("🔗 TXID: {}", tx.compute_txid());
                    println!("📏 Size: {} bytes", tx_bytes.len());
                    println!("⚖️  vSize: {} vbytes", tx.vsize());
                    println!("⚖️  Weight: {} WU", tx.weight().to_wu());
                    println!("\n📥 Inputs ({}):", tx.input.len());
                    for (i, input) in tx.input.iter().enumerate() {
                        println!("  Input #{i}:");
                        println!("    Prev TXID: {}", input.previous_output.txid);
                        println!("    Prev Vout: {}", input.previous_output.vout);
                        println!("    Sequence: {}", input.sequence.0);
                        println!("    Witness items: {}", input.witness.len());
                        for (j, item) in input.witness.iter().enumerate() {
                            println!("      Witness[{j}]: {} bytes", item.len());
                        }
                    }
                    println!("\n📤 Outputs ({}):", tx.output.len());
                    for (i, output) in tx.output.iter().enumerate() {
                        println!("  Output #{i}:");
                        println!("    Value: {} sats", output.value.to_sat());
                        println!("    Script: {} bytes", output.script_pubkey.len());
                    }
                }
                Ok(())
            },
            BitcoindCommands::Getchaintips { raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_chain_tips(provider).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Getmempoolinfo { raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_mempool_info(provider).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Getrawmempool { raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_raw_mempool(provider).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Gettxout { txid, vout, include_mempool, raw } => {
                let result = <ConcreteProvider as BitcoinRpcProvider>::get_tx_out(provider, &txid, vout, include_mempool).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            BitcoindCommands::Sendrawtransaction { tx_hex, from_file, use_slipstream, use_rebar, raw } => {
                // Read hex from file or argument
                let hex_string = if let Some(file_path) = from_file {
                    std::fs::read_to_string(file_path)
                        .map_err(|e| AlkanesError::Storage(format!("Failed to read file: {}", e)))?
                        .trim()
                        .to_string()
                } else {
                    tx_hex.ok_or_else(|| AlkanesError::InvalidParameters("No transaction hex or file provided".to_string()))?
                };
                
                let result = if use_rebar {
                    // Use Rebar Shield service
                    eprintln!("🔒 Using Rebar Shield for private transaction broadcast...");
                    eprintln!("   Endpoint: https://shield.rebarlabs.io/v1/rpc");
                    eprintln!("   Note: Transaction must include Rebar payment output");
                    eprintln!("");
                    
                    use alkanes_cli_common::provider::rebar;
                    rebar::submit_transaction(&hex_string).await
                        .map_err(|e| AlkanesError::Network(format!("Rebar Shield error: {}", e)))?
                } else if use_slipstream {
                    // Use MARA Slipstream service
                    eprintln!("🚀 Using MARA Slipstream for transaction broadcast...");
                    eprintln!("   Endpoint: https://slipstream.mara.com/rest-api/submit-tx");
                    eprintln!("   Note: Minimum fee rate is 2 sats/vByte");
                    eprintln!("");
                    
                    let client = reqwest::Client::new();
                    let payload = serde_json::json!({
                        "tx_hex": hex_string
                    });
                    
                    let response = client
                        .post("https://slipstream.mara.com/rest-api/submit-tx")
                        .header("Content-Type", "application/json")
                        .json(&payload)
                        .send()
                        .await
                        .map_err(|e| AlkanesError::Network(format!("Slipstream request failed: {}", e)))?;
                    
                    let status = response.status();
                    let response_text = response.text().await
                        .map_err(|e| AlkanesError::Network(format!("Failed to read Slipstream response: {}", e)))?;
                    
                    if !status.is_success() {
                        return Err(AlkanesError::Network(format!("Slipstream error ({}): {}", status, response_text)));
                    }
                    
                    let response_json: serde_json::Value = serde_json::from_str(&response_text)
                        .map_err(|e| AlkanesError::Network(format!("Failed to parse Slipstream response: {}", e)))?;
                    
                    // Extract txid from response (in "message" field according to API spec)
                    if let Some(txid) = response_json.get("message").and_then(|v| v.as_str()) {
                        txid.to_string()
                    } else {
                        // Return full response if no message found
                        response_text
                    }
                } else {
                    // Use standard Bitcoin RPC
                    <ConcreteProvider as BitcoinRpcProvider>::send_raw_transaction(provider, &hex_string).await?
                };
                
                if raw {
                    println!("{result}");
                } else {
                    if use_slipstream {
                        println!("✅ Transaction submitted to MARA Slipstream successfully!");
                        println!("🔗 Transaction ID: {result}");
                        println!("💡 Your transaction will be included in the next MARA-mined block");
                    } else {
                        println!("✅ Transaction broadcast successfully!");
                        println!("🔗 Transaction ID: {result}");
                    }
                }
                Ok(())
            },
       };
       res.map_err(|e| AlkanesError::Wallet(e.to_string()))
   }
}

#[async_trait(?Send)]
impl SystemMetashrew for SystemAlkanes {
   async fn execute_metashrew_command(&self, command: MetashrewCommands) -> alkanes_cli_common::Result<()> {
       let provider = &self.provider;
       let res: anyhow::Result<()> = match command {
            MetashrewCommands::Height => {
                let height = provider.get_metashrew_height().await?;
                println!("{height}");
                Ok(())
            },
            MetashrewCommands::Getstateroot { height, raw } => {
                let state_root = <ConcreteProvider as alkanes_cli_common::MetashrewRpcProvider>::get_state_root(provider, serde_json::json!(height)).await?;
                if raw {
                    println!("{}", serde_json::json!(state_root));
                } else {
                    println!("{state_root}");
                }
                Ok(())
            },
       };
       res.map_err(|e| AlkanesError::Wallet(e.to_string()))
   }
}

#[async_trait(?Send)]
impl alkanes_cli_common::SystemAlkanes for SystemAlkanes {
    async fn execute_alkanes_command(&self, command: AlkanesCommands) -> alkanes_cli_common::Result<()> {
        let mut provider = self.provider.clone();

        if command.requires_signing() {
            if let alkanes_cli_common::provider::WalletState::Locked(_) = provider.get_wallet_state() {
                let passphrase = if let Some(ref pass) = self.args.passphrase {
                    pass.clone()
                } else {
                    rpassword::prompt_password("Enter passphrase to unlock keystore for signing: ")
                        .map_err(|e| AlkanesError::Wallet(format!("Failed to get passphrase: {e}")))?
                };
                provider.unlock_wallet(&passphrase).await?;
            } else if let alkanes_cli_common::provider::WalletState::None = provider.get_wallet_state() {
                return Err(AlkanesError::Wallet("No wallet found. Please create or specify a wallet file.".to_string()));
            }
        }

        let res: anyhow::Result<()> = match command {
            AlkanesCommands::Execute {
                fee_rate,
                inputs,
                to,
                change,
                protostones,
                envelope,
                raw,
                trace,
                mine,
                yes,
            } => {
                log::info!("🚀 Starting enhanced alkanes execute command");

                // Resolve change address if provided
                let resolved_change = if let Some(change_addr) = change {
                    Some(provider.resolve_all_identifiers(&change_addr).await?)
                } else {
                    None
                };

                // Load envelope data if provided
                let envelope_data = if let Some(ref envelope_file) = envelope {
                    let expanded_path = expand_tilde(envelope_file)?;
                    let data = std::fs::read(&expanded_path)
                        .with_context(|| format!("Failed to read envelope file: {expanded_path}"))?;
                    log::info!("📦 Loaded envelope data: {} bytes", data.len());
                    Some(data)
                } else {
                    None
                };

                // Parse input requirements from a single string
                let parsed_input_requirements = {
                    use alkanes_cli_common::alkanes::parsing::parse_input_requirements;
                    parse_input_requirements(&inputs)
                        .map_err(|e| anyhow!("Failed to parse input requirements: {}", e))?
                };

                // Parse protostones from a single string
                let parsed_protostones = {
                    use alkanes_cli_common::alkanes::parsing::parse_protostones;
                    parse_protostones(&protostones)
                        .map_err(|e| anyhow!("Failed to parse protostones: {}", e))?
                };

                // Resolve 'to' addresses from a comma-separated string
                let resolved_to_addresses = {
                    let mut resolved = Vec::new();
                    for addr in to.split(',') {
                        resolved.push(provider.resolve_all_identifiers(addr.trim()).await?);
                    }
                    resolved
                };

                 // Create enhanced execute parameters
                 let execute_params = alkanes_cli_common::alkanes::types::EnhancedExecuteParams {
                     fee_rate,
                     to_addresses: resolved_to_addresses,
                     from_addresses: None, // This field is no longer provided by the CLI
                     change_address: resolved_change,
                     input_requirements: parsed_input_requirements,
                     protostones: parsed_protostones,
                    envelope_data,
                    raw_output: raw,
                    trace_enabled: trace,
                    mine_enabled: mine,
                    auto_confirm: yes, // Use the new field name
                };

                let mut current_state = provider.execute(execute_params.clone()).await?;

                loop {
                    match current_state {
                        alkanes_cli_common::alkanes::types::ExecutionState::ReadyToSign(state) => {
                            if !yes {
                                pretty_print::pretty_print_ready_to_sign(&state);
                                println!("Do you want to sign and broadcast this transaction? (y/N)");
                                let mut input = String::new();
                                std::io::stdin().read_line(&mut input)?;
                                if !input.trim().to_lowercase().starts_with('y') {
                                    println!("❌ Transaction cancelled.");
                                    return Ok(());
                                }
                            }
                            let result = provider.resume_execution(state, &execute_params).await?;
                            current_state = alkanes_cli_common::alkanes::types::ExecutionState::Complete(result);
                        }
                        alkanes_cli_common::alkanes::types::ExecutionState::ReadyToSignCommit(state) => {
                             if !yes {
                                println!("A commit transaction is ready to be signed and broadcast.");
                                println!("Do you want to proceed? (y/N)");
                                let mut input = String::new();
                                std::io::stdin().read_line(&mut input)?;
                                if !input.trim().to_lowercase().starts_with('y') {
                                    println!("❌ Transaction cancelled.");
                                    return Ok(());
                                }
                            }
                            current_state = provider.resume_commit_execution(state).await?;
                        }
                        alkanes_cli_common::alkanes::types::ExecutionState::ReadyToSignReveal(state) => {
                            if !yes {
                                pretty_print::pretty_print_reveal_analysis(&state);
                                println!("A reveal transaction is ready to be signed and broadcast.");
                                println!("Do you want to proceed? (y/N)");
                                let mut input = String::new();
                                std::io::stdin().read_line(&mut input)?;
                                if !input.trim().to_lowercase().starts_with('y') {
                                    println!("❌ Transaction cancelled.");
                                    return Ok(());
                                }
                            }
                            let result = provider.resume_reveal_execution(state).await?;
                            current_state = alkanes_cli_common::alkanes::types::ExecutionState::Complete(result);
                        }
                        alkanes_cli_common::alkanes::types::ExecutionState::Complete(result) => {
                            if raw {
                                println!("{}", serde_json::to_string_pretty(&result)?);
                            } else {
                                println!("✅ Alkanes execution completed successfully!");
                                if let Some(commit_txid) = result.commit_txid {
                                    println!("🔗 Commit TXID: {commit_txid}");
                                }
                                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                                if let Some(commit_fee) = result.commit_fee {
                                    println!("💰 Commit Fee: {commit_fee} sats");
                                }
                                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                                if let Some(traces) = result.traces {
                                    for (i, trace) in traces.iter().enumerate() {
                                        println!("
📊 Trace for protostone #{}:", i + 1);
                                        println!("{}", serde_json::to_string_pretty(&trace).unwrap_or_else(|_| format!("{trace:#?}")));
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
                Ok(())
            }
            AlkanesCommands::Balance { address, raw } => {
                let balance_result = alkanes_cli_common::AlkanesProvider::get_balance(&provider, address.as_deref()).await?;

                if raw {
                    println!("{}", serde_json::to_string_pretty(&balance_result)?);
                } else {
                    println!("🪙 Alkanes Balances");
                    println!("═══════════════════");
                    println!("{}", serde_json::to_string_pretty(&balance_result)?);
                }
                Ok(())
            }
            AlkanesCommands::Trace { outpoint, raw } => {
                let (txid, vout) = parse_outpoint(&outpoint)?;
                let trace_result = provider.trace_outpoint(&txid, vout).await?;

                if raw {
                    println!("{}", serde_json::to_string_pretty(&trace_result)?);
                } else {
                    println!("📊 Alkanes Transaction Trace");
                    println!("═══════════════════════════");
                    println!("{}", serde_json::to_string_pretty(&trace_result)?);
                }
                Ok(())
            }
            AlkanesCommands::Inspect {
                target,
                raw,
                disasm,
                fuzz,
                fuzz_ranges,
                meta,
                codehash,
            } => {
                let config = AlkanesInspectConfig {
                    disasm,
                    fuzz,
                    fuzz_ranges,
                    meta,
                    codehash,
                    raw,
                };

                let result = provider.inspect(&target, config).await?;

                if raw {
                    // Convert to serializable format
                    let serializable_result = serde_json::json!({
                        "alkane_id": {
                            "block": result.alkane_id.block,
                            "tx": result.alkane_id.tx
                        },
                        "bytecode_length": result.bytecode_length,
                        "disassembly": result.disassembly,
                        "metadata": result.metadata,
                        "metadata_error": result.metadata_error,
                        "codehash": result.codehash,
                        "fuzzing_results": result.fuzzing_results
                    });
                    println!("{}", serde_json::to_string_pretty(&serializable_result)?);
                } else {
                    pretty_print::pretty_print_inspection_result(&result)?;
                }
                Ok(())
            }
            AlkanesCommands::Getbytecode { alkane_id, raw, block_tag } => {
                let bytecode = AlkanesProvider::get_bytecode(&provider, &alkane_id, block_tag).await?;

                if raw {
                    let json_result = serde_json::json!({
                        "alkane_id": alkane_id,
                        "bytecode": bytecode
                    });
                    println!("{}", serde_json::to_string_pretty(&json_result)?);
                } else {
                    println!("🔍 Alkanes Contract Bytecode");
                    println!("═══════════════════════════");
                    println!("🏷️  Alkane ID: {alkane_id}");

                    if bytecode.is_empty() || bytecode == "0x" {
                        println!("❌ No bytecode found for this contract");
                    } else {
                        // Remove 0x prefix if present for display
                        let clean_bytecode = bytecode.strip_prefix("0x").unwrap_or(&bytecode);

                        println!("💾 Bytecode:");
                        println!("   Length: {} bytes", clean_bytecode.len() / 2);
                        println!("   Hex: {bytecode}");

                        // Show first few bytes for quick inspection
                        if clean_bytecode.len() >= 8 {
                            println!("   First 4 bytes: {}", &clean_bytecode[..8]);
                        }
                    }
                }
                Ok(())
            }
            AlkanesCommands::Simulate {
                contract_id,
                params,
                block_hex,
                transaction_hex,
                raw,
            } => {
                use prost::Message;
                use alkanes_cli_common::params_parser::{parse_params, parse_alkane_id};
                use alkanes_cli_common::proto::alkanes as alkanes_pb;
                
                // Get current height from metashrew
                let height = provider.get_metashrew_height().await?;
                
                // Parse contract_id (format: block:tx)
                let alkane_id = parse_alkane_id(&contract_id)?;
                
                // Build MessageContextParcel
                let mut parcel = alkanes_pb::MessageContextParcel {
                    height,
                    vout: 2,
                    pointer: 0,
                    refund_pointer: 0,
                    txindex: 0,
                    transaction: vec![],
                    block: vec![],
                    calldata: vec![],
                    alkanes: vec![],
                };
                
                // Set block_hex if provided
                if let Some(ref hex) = block_hex {
                    parcel.block = hex::decode(hex.trim_start_matches("0x"))?;
                }
                
                // Set transaction_hex if provided
                if let Some(ref hex) = transaction_hex {
                    parcel.transaction = hex::decode(hex.trim_start_matches("0x"))?;
                }
                
                // Parse params if provided (format: [block,tx,inputs...]:[block:tx:value]:[block:tx:value])
                if let Some(ref params_str) = params {
                    let (cellpack, alkane_parcel) = parse_params(params_str)?;
                    
                    // Encode calldata using Cellpack::encipher
                    parcel.calldata = cellpack.encipher();
                    
                    // Convert AlkaneTransferParcel to protobuf format
                    for transfer in alkane_parcel.0 {
                        let mut transfer_pb = alkanes_pb::AlkaneTransfer::default();
                        
                        let mut id_pb = alkanes_pb::AlkaneId::default();
                        let mut block_uint128 = alkanes_pb::Uint128::default();
                        block_uint128.lo = transfer.id.block as u64;
                        block_uint128.hi = (transfer.id.block >> 64) as u64;
                        id_pb.block = Some(block_uint128).into();
                        
                        let mut tx_uint128 = alkanes_pb::Uint128::default();
                        tx_uint128.lo = transfer.id.tx as u64;
                        tx_uint128.hi = (transfer.id.tx >> 64) as u64;
                        id_pb.tx = Some(tx_uint128).into();
                        
                        transfer_pb.id = Some(id_pb).into();
                        
                        let mut value_uint128 = alkanes_pb::Uint128::default();
                        value_uint128.lo = transfer.value as u64;
                        value_uint128.hi = (transfer.value >> 64) as u64;
                        transfer_pb.value = Some(value_uint128).into();
                        
                        parcel.alkanes.push(transfer_pb);
                    }
                }
                
                // Encode the parcel
                let hex_input = format!("0x{}", hex::encode(parcel.encode_to_vec()));
                
                // Build the view function path: "block:tx/simulate"
                let view_fn = format!("{}:{}/simulate", alkane_id.block, alkane_id.tx);
                
                // Call metashrew_view
                let response_bytes = provider.metashrew_view_call(&view_fn, &hex_input, "latest").await?;
                
                // Decode response as SimulateResponse
                let simulate_response = alkanes_pb::SimulateResponse::decode(response_bytes.as_slice())?;
                
                if raw {
                    println!("{:#?}", simulate_response);
                } else {
                    println!("🧪 Alkanes Contract Simulation");
                    println!("═══════════════════════════");
                    println!("🔗 Contract ID: {contract_id}");
                    println!("⛽ Gas Used: {}", simulate_response.gas_used);
                    if !simulate_response.error.is_empty() {
                        println!("❌ Error: {}", simulate_response.error);
                    }
                    if let Some(ref execution) = simulate_response.execution {
                        println!("📊 Execution Result:");
                        println!("   Return data: 0x{}", hex::encode(&execution.data));
                        println!("   Alkanes transferred: {}", execution.alkanes.len());
                        for (i, alkane) in execution.alkanes.iter().enumerate() {
                            if let Some(ref id) = alkane.id {
                                let block = id.block.as_ref().map(|b| b.lo).unwrap_or(0);
                                let tx = id.tx.as_ref().map(|t| t.lo).unwrap_or(0);
                                let value = alkane.value.as_ref().map(|v| v.lo).unwrap_or(0);
                                println!("   {}. Alkane {}:{} -> {} units", i + 1, block, tx, value);
                            }
                        }
                    }
                }
                Ok(())
            }
            AlkanesCommands::GetBlock { height, raw } => {
                let result = AlkanesProvider::get_block(&provider, height).await?;
                if raw {
                    println!("{result:#?}");
                } else {
                    println!("📦 Alkanes Block {height}:
{result:#?}");
                }
                Ok(())
            }
            AlkanesCommands::Sequence { raw } => {
                let result = provider.sequence().await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("🔢 Sequence:
{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            }
            AlkanesCommands::SpendablesByAddress { address, raw } => {
                let result = provider.spendables_by_address(&address).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("💰 Spendables for {}:
{}", address, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            }
            AlkanesCommands::TraceBlock { height, raw } => {
                let result = provider.trace_block(height).await?;
                if raw {
                    println!("{result:#?}");
                } else {
                    println!("📊 Trace for block {height}:
{result:#?}");
                }
                Ok(())
            }
            AlkanesCommands::Getstorage { alkane_id, path, block_tag, raw } => {
                use prost::Message;
                use alkanes_cli_common::params_parser::parse_alkane_id;
                use alkanes_cli_common::proto::alkanes as alkanes_pb;
                
                // Parse alkane_id (format: block:tx)
                let id = parse_alkane_id(&alkane_id)?;
                
                // Build AlkaneStorageRequest
                let mut request = alkanes_pb::AlkaneStorageRequest::default();
                let mut id_pb = alkanes_pb::AlkaneId::default();
                
                let mut block_uint128 = alkanes_pb::Uint128::default();
                block_uint128.lo = id.block as u64;
                block_uint128.hi = (id.block >> 64) as u64;
                id_pb.block = Some(block_uint128).into();
                
                let mut tx_uint128 = alkanes_pb::Uint128::default();
                tx_uint128.lo = id.tx as u64;
                tx_uint128.hi = (id.tx >> 64) as u64;
                id_pb.tx = Some(tx_uint128).into();
                
                request.id = Some(id_pb).into();
                request.path = hex::decode(path.trim_start_matches("0x"))?;
                
                let hex_input = format!("0x{}", hex::encode(request.encode_to_vec()));
                let response_bytes = provider.metashrew_view_call("getstorage", &hex_input, block_tag.as_deref().unwrap_or("latest")).await?;
                
                let storage_response = alkanes_pb::AlkaneStorageResponse::decode(response_bytes.as_slice())?;
                
                if raw {
                    println!("{{\"value\": \"0x{}\"}}", hex::encode(&storage_response.value));
                } else {
                    println!("🗄️  Storage Value for {alkane_id}:");
                    println!("   Path: {path}");
                    println!("   Value: 0x{}", hex::encode(&storage_response.value));
                }
                Ok(())
            }
            AlkanesCommands::Getinventory { outpoint, block_tag, raw } => {
                use prost::Message;
                use alkanes_cli_common::params_parser::parse_outpoint;
                use alkanes_cli_common::proto::alkanes as alkanes_pb;
                
                // Parse outpoint (format: txid:vout)
                let (txid, vout) = parse_outpoint(&outpoint)?;
                
                // Build AlkaneInventoryRequest (which uses Outpoint)
                let mut request = alkanes_pb::Outpoint::default();
                request.txid = hex::decode(&txid)?;
                request.vout = vout;
                
                let hex_input = format!("0x{}", hex::encode(request.encode_to_vec()));
                let response_bytes = provider.metashrew_view_call("getinventory", &hex_input, block_tag.as_deref().unwrap_or("latest")).await?;
                
                let inventory_response = alkanes_pb::AlkaneInventoryResponse::decode(response_bytes.as_slice())?;
                
                if raw {
                    println!("{:#?}", inventory_response);
                } else {
                    println!("📦 Alkane Inventory at {outpoint}:");
                    for (i, alkane) in inventory_response.alkanes.iter().enumerate() {
                        if let Some(ref id) = alkane.id {
                            let block = id.block.as_ref().map(|b| b.lo).unwrap_or(0);
                            let tx = id.tx.as_ref().map(|t| t.lo).unwrap_or(0);
                            let value = alkane.value.as_ref().map(|v| v.lo).unwrap_or(0);
                            println!("   {}. Alkane {}:{} -> {} units", i + 1, block, tx, value);
                        }
                    }
                }
                Ok(())
            }
            AlkanesCommands::WrapBtc { amount, from, change, fee_rate, raw, trace, mine, yes } => {
                use alkanes_cli_common::alkanes::wrap_btc::{WrapBtcExecutor, WrapBtcParams};
                
                let params = WrapBtcParams {
                    amount,
                    from_addresses: from,
                    change_address: change,
                    fee_rate,
                    raw_output: raw,
                    trace_enabled: trace,
                    mine_enabled: mine,
                    auto_confirm: yes,
                };

                let mut executor = WrapBtcExecutor::new(&mut provider);
                let result = executor.wrap_btc(params).await?;

                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("✅ BTC wrapped successfully!");
                    println!("🔗 Commit TXID: {}", result.commit_txid.as_ref().unwrap_or(&"N/A".to_string()));
                    println!("🔗 Reveal TXID: {}", result.reveal_txid);
                    println!("💰 Commit Fee: {} sats", result.commit_fee.unwrap_or(0));
                    println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                    println!("🎉 frBTC minted and locked in vault!");
                }
                Ok(())
            }
        };
        res.map_err(|e| AlkanesError::Wallet(e.to_string()))
    }
}

use alkanes_cli_common::runestone_analysis::{
    analyze_transaction_with_runestone, pretty_print_transaction_analysis,
};
use bitcoin::Transaction;
use bitcoin::consensus::deserialize;

fn decode_transaction_hex(hex_str: &str) -> anyhow::Result<Transaction> {
    let tx_bytes = hex::decode(hex_str.trim_start_matches("0x"))
        .context("Failed to decode transaction hex")?;
    
    let tx: Transaction = deserialize(&tx_bytes)
        .context("Failed to deserialize transaction")?;
    
    Ok(tx)
}

#[async_trait(?Send)]
impl SystemRunestone for SystemAlkanes {
    async fn execute_runestone_command(&self, command: RunestoneCommands) -> alkanes_cli_common::Result<()> {
        let provider = &self.provider;
        let res: anyhow::Result<()> = match command {
            RunestoneCommands::Decode { tx_hex, raw } => {
                let tx = decode_transaction_hex(&tx_hex)?;
                let network = provider.get_network();
                let analysis = analyze_transaction_with_runestone(&tx, network)?;

                if raw {
                    println!("{}", serde_json::to_string_pretty(&analysis)?);
                } else {
                    let pretty_output = pretty_print_transaction_analysis(&analysis)?;
                    println!("{pretty_output}");
                }
                Ok(())
            },
            RunestoneCommands::Analyze { txid, raw } => {
                let tx_hex = provider.get_transaction_hex(&txid).await?;
                let tx = decode_transaction_hex(&tx_hex)?;
                let network = provider.get_network();
                let analysis = analyze_transaction_with_runestone(&tx, network)?;

                if raw {
                    println!("{}", serde_json::to_string_pretty(&analysis)?);
                } else {
                    let pretty_output = pretty_print_transaction_analysis(&analysis)?;
                    println!("{pretty_output}");
                }
                Ok(())
            },
        };
        res.map_err(|e| AlkanesError::Wallet(e.to_string()))
    }
}

#[async_trait(?Send)]
impl SystemProtorunes for SystemAlkanes {
   async fn execute_protorunes_command(&self, command: ProtorunesCommands) -> alkanes_cli_common::Result<()> {
       let provider = &self.provider;
       let res: anyhow::Result<()> = match command {
            ProtorunesCommands::Byaddress { address, raw, block_tag, protocol_tag } => {
                let result = provider.get_protorunes_by_address(&address, block_tag, protocol_tag).await?;
                
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("🪙 Protorunes for address: {address}");
                    println!("═══════════════════════════════════════");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
           ProtorunesCommands::Byoutpoint { txid, vout, raw, block_tag, protocol_tag } => {
               let result = provider.get_protorunes_by_outpoint(&txid, vout, block_tag, protocol_tag).await?;
               
               if raw {
                   println!("{}", serde_json::to_string_pretty(&result)?);
               } else {
                   println!("🪙 Protorunes for outpoint: {txid}:{vout}");
                   println!("═══════════════════════════════════════");
                   println!("{}", serde_json::to_string_pretty(&result)?);
               }
               Ok(())
           },
       };
       res.map_err(|e| AlkanesError::Wallet(e.to_string()))
   }
}

#[async_trait(?Send)]
impl SystemMonitor for SystemAlkanes {
   async fn execute_monitor_command(&self, command: MonitorCommands) -> alkanes_cli_common::Result<()> {
       let provider = &self.provider;
       let res: anyhow::Result<()> = match command {
            MonitorCommands::Blocks { start, raw: _ } => {
                let start_height = start.unwrap_or({
                    // Get current height as default
                    0 // Placeholder - would need async context
                });
                
                println!("🔍 Monitoring blocks starting from height: {start_height}");
                provider.monitor_blocks(start).await?;
                println!("✅ Block monitoring completed");
                Ok(())
            },
       };
       res.map_err(|e| AlkanesError::Wallet(e.to_string()))
   }
}

#[async_trait(?Send)]
impl SystemEsplora for SystemAlkanes {
    async fn execute_esplora_command(&self, command: EsploraCommands) -> alkanes_cli_common::Result<()> {
        let provider = &self.provider;
        let res: anyhow::Result<()> = match command {
            EsploraCommands::BlocksTipHash { raw } => {
                let hash = provider.get_blocks_tip_hash().await?;
                if raw {
                    println!("{hash}");
                } else {
                    println!("⛓️ Tip Hash: {hash}");
                }
                Ok(())
            },
            EsploraCommands::BlocksTipHeight { raw } => {
                let height = provider.get_blocks_tip_height().await?;
                if raw {
                    println!("{height}");
                } else {
                    println!("📈 Tip Height: {height}");
                }
                Ok(())
            },
            EsploraCommands::Blocks { start_height, raw } => {
                let result = provider.get_blocks(start_height).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("📦 Blocks:
{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::BlockHeight { height, raw } => {
                let hash = provider.get_block_by_height(height).await?;
                if raw {
                    println!("{hash}");
                } else {
                    println!("🔗 Block Hash at {height}: {hash}");
                }
                Ok(())
            },
            EsploraCommands::Block { hash, raw } => {
                let block = EsploraProvider::get_block(provider, &hash).await?;
                if raw {
                    if let Some(s) = block.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{block}");
                    }
                } else {
                    println!("📦 Block {}:
{}", hash, serde_json::to_string_pretty(&block)?);
                }
                Ok(())
            },
            EsploraCommands::BlockStatus { hash, raw } => {
                let status = provider.get_block_status(&hash).await?;
                if raw {
                    if let Some(s) = status.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{status}");
                    }
                } else {
                    println!("ℹ️ Block Status {}:
{}", hash, serde_json::to_string_pretty(&status)?);
                }
                Ok(())
            },
            EsploraCommands::BlockTxids { hash, raw } => {
                let txids = provider.get_block_txids(&hash).await?;
                if raw {
                    if let Some(s) = txids.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{txids}");
                    }
                } else {
                    println!("📄 Block Txids {}:
{}", hash, serde_json::to_string_pretty(&txids)?);
                }
                Ok(())
            },
            EsploraCommands::BlockHeader { hash, raw } => {
                let header = <ConcreteProvider as EsploraProvider>::get_block_header(provider, &hash).await?;
                if raw {
                    println!("{header}");
                } else {
                    println!("📄 Block Header {hash}: {header}");
                }
                Ok(())
            },
            EsploraCommands::BlockRaw { hash, raw } => {
                let raw_block = provider.get_block_raw(&hash).await?;
                if raw {
                    println!("{raw_block}");
                } else {
                    println!("📦 Raw Block {hash}: {raw_block}");
                }
                Ok(())
            },
            EsploraCommands::BlockTxid { hash, index, raw } => {
                let txid = provider.get_block_txid(&hash, index).await?;
                if raw {
                    println!("{txid}");
                } else {
                    println!("📄 Txid at index {index} in block {hash}: {txid}");
                }
                Ok(())
            },
            EsploraCommands::BlockTxs { hash, start_index, raw } => {
                let txs = provider.get_block_txs(&hash, start_index).await?;
                if raw {
                    if let Some(s) = txs.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{txs}");
                    }
                } else {
                    println!("📄 Transactions in block {}:
{}", hash, serde_json::to_string_pretty(&txs)?);
                }
                Ok(())
            },
            EsploraCommands::Address { params, raw } => {
                let resolved_params = provider.resolve_all_identifiers(&params).await?;
                let result = EsploraProvider::get_address_info(provider, &resolved_params).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("🏠 Address {}:
{}", params, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::AddressTxs { params, raw } => {
                let resolved_params = provider.resolve_all_identifiers(&params).await?;
                let result = provider.get_address_txs(&resolved_params).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("📄 Transactions for address {}:
{}", params, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::AddressTxsChain { params, raw } => {
                let parts: Vec<&str> = params.split(':').collect();
                let resolved_params = if parts.len() >= 2 {
                    let address_part = parts[0];
                    let resolved_address = provider.resolve_all_identifiers(address_part).await?;
                    if parts.len() == 2 {
                        format!("{}:{}", resolved_address, parts[1])
                    } else {
                        format!("{}:{}", resolved_address, parts[1..].join(":"))
                    }
                } else {
                    provider.resolve_all_identifiers(&params).await?
                };
                let result = provider.get_address_txs_chain(&resolved_params, None).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("⛓️ Chain transactions for address {}:
{}", params, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::AddressTxsMempool { address, raw } => {
                let resolved_address = provider.resolve_all_identifiers(&address).await?;
                let result = provider.get_address_txs_mempool(&resolved_address).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("⏳ Mempool transactions for address {}:
{}", address, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::AddressUtxo { address, raw } => {
                let resolved_address = provider.resolve_all_identifiers(&address).await?;
                let result = provider.get_address_utxo(&resolved_address).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("💰 UTXOs for address {}:
{}", address, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::AddressPrefix { prefix, raw } => {
                let result = provider.get_address_prefix(&prefix).await?;
                if raw {
                    if let Some(s) = result.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{result}");
                    }
                } else {
                    println!("🔍 Addresses with prefix '{}':
{}", prefix, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            EsploraCommands::Tx { txid, raw } => {
                let tx = provider.get_tx(&txid).await?;
                if raw {
                    if let Some(s) = tx.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{tx}");
                    }
                } else {
                    println!("📄 Transaction {}:
{}", txid, serde_json::to_string_pretty(&tx)?);
                }
                Ok(())
            },
            EsploraCommands::TxHex { txid, raw } => {
                let hex = provider.get_tx_hex(&txid).await?;
                if raw {
                    println!("{hex}");
                } else {
                    println!("📄 Hex for tx {txid}: {hex}");
                }
                Ok(())
            },
            EsploraCommands::TxRaw { txid, raw } => {
                let raw_tx = provider.get_tx_raw(&txid).await?;
                if raw {
                    println!("{raw_tx}");
                } else {
                    println!("📄 Raw tx {txid}: {raw_tx}");
                }
                Ok(())
            },
            EsploraCommands::TxStatus { txid, raw } => {
                let status = provider.get_tx_status(&txid).await?;
                if raw {
                    if let Some(s) = status.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{status}");
                    }
                } else {
                    println!("ℹ️ Status for tx {}:
{}", txid, serde_json::to_string_pretty(&status)?);
                }
                Ok(())
            },
            EsploraCommands::TxMerkleProof { txid, raw } => {
                let proof = provider.get_tx_merkle_proof(&txid).await?;
                if raw {
                    if let Some(s) = proof.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{proof}");
                    }
                } else {
                    println!("🧾 Merkle proof for tx {}:
{}", txid, serde_json::to_string_pretty(&proof)?);
                }
                Ok(())
            },
            EsploraCommands::TxMerkleblockProof { txid, raw } => {
                let proof = provider.get_tx_merkleblock_proof(&txid).await?;
                if raw {
                    println!("{proof}");
                } else {
                    println!("🧾 Merkleblock proof for tx {txid}: {proof}");
                }
                Ok(())
            },
            EsploraCommands::TxOutspend { txid, index, raw } => {
                let outspend = provider.get_tx_outspend(&txid, index).await?;
                if raw {
                    if let Some(s) = outspend.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{outspend}");
                    }
                } else {
                    println!("💸 Outspend for tx {}, vout {}:
{}", txid, index, serde_json::to_string_pretty(&outspend)?);
                }
                Ok(())
            },
            EsploraCommands::TxOutspends { txid, raw } => {
                let outspends = provider.get_tx_outspends(&txid).await?;
                if raw {
                    if let Some(s) = outspends.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{outspends}");
                    }
                } else {
                    println!("💸 Outspends for tx {}:
{}", txid, serde_json::to_string_pretty(&outspends)?);
                }
                Ok(())
            },
            EsploraCommands::Broadcast { tx_hex, raw: _ } => {
                let txid = provider.broadcast(&tx_hex).await?;
                println!("✅ Transaction broadcast successfully!");
                println!("🔗 Transaction ID: {txid}");
                Ok(())
            },
            EsploraCommands::PostTx { tx_hex, raw: _ } => {
                let txid = provider.broadcast(&tx_hex).await?;
                println!("✅ Transaction posted successfully!");
                println!("🔗 Transaction ID: {txid}");
                Ok(())
            },
            EsploraCommands::Mempool { raw } => {
                let mempool = provider.get_mempool().await?;
                if raw {
                    if let Some(s) = mempool.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{mempool}");
                    }
                } else {
                    println!("⏳ Mempool Info:
{}", serde_json::to_string_pretty(&mempool)?);
                }
                Ok(())
            },
            EsploraCommands::MempoolTxids { raw } => {
                let txids = provider.get_mempool_txids().await?;
                if raw {
                    if let Some(s) = txids.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{txids}");
                    }
                } else {
                    println!("📄 Mempool Txids:
{}", serde_json::to_string_pretty(&txids)?);
                }
                Ok(())
            },
            EsploraCommands::MempoolRecent { raw } => {
                let recent = provider.get_mempool_recent().await?;
                if raw {
                    if let Some(s) = recent.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{recent}");
                    }
                } else {
                    println!("📄 Recent Mempool Txs:
{}", serde_json::to_string_pretty(&recent)?);
                }
                Ok(())
            },
            EsploraCommands::FeeEstimates { raw } => {
                let estimates = provider.get_fee_estimates().await?;
                if raw {
                    if let Some(s) = estimates.as_str() {
                        println!("{}", s.trim_matches('"'));
                    } else {
                        println!("{estimates}");
                    }
                } else {
                    println!("💰 Fee Estimates:
{}", serde_json::to_string_pretty(&estimates)?);
                }
                Ok(())
            },
        };
        res.map_err(|e| AlkanesError::Wallet(e.to_string()))
    }
}


#[async_trait(?Send)]
pub trait SystemOrd {
    async fn execute_ord_command(&self, command: OrdCommands) -> alkanes_cli_common::Result<()>;
}

#[async_trait(?Send)]
impl SystemOrd for SystemAlkanes {
    async fn execute_ord_command(&self, command: OrdCommands) -> alkanes_cli_common::Result<()> {
        let provider = &self.provider;
        let res: anyhow::Result<()> = match command {
            OrdCommands::Inscription { id, raw } => {
                let inscription = provider.get_inscription(&id).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&inscription)?);
                } else {
                    println!("Inscription {}:
{}", id, serde_json::to_string_pretty(&inscription)?);
                }
                Ok(())
            },
            OrdCommands::InscriptionsInBlock { hash, raw } => {
                let inscriptions = provider.get_inscriptions_in_block(&hash).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&inscriptions)?);
                } else {
                    println!("Inscriptions in block {}:
{}", hash, serde_json::to_string_pretty(&inscriptions)?);
                }
                Ok(())
            },
            OrdCommands::AddressInfo { address, raw } => {
                let result = provider.get_ord_address_info(&address).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Address {}:
{}", address, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::BlockInfo { query, raw } => {
                let result = provider.get_block_info(&query).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Block {}:
{}", query, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::BlockCount => {
                let result = provider.get_ord_block_count().await?;
                println!("Block count:
{}", serde_json::to_string_pretty(&result)?);
                Ok(())
            },
            OrdCommands::Blocks { raw } => {
                let result = provider.get_ord_blocks().await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Blocks:
{}", serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::Children { id, page, raw } => {
                let result = provider.get_children(&id, page).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Children of {}:
{}", id, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::Content { id } => {
                let result = provider.get_content(&id).await?;
                use std::io::{self, Write};
                io::stdout().write_all(&result)?;
                Ok(())
            },
            OrdCommands::Output { outpoint, raw } => {
                let result = provider.get_output(&outpoint).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Output {}:
{}", outpoint, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::Parents { id, page, raw } => {
                let result = provider.get_parents(&id, page).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Parents of {}:
{}", id, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::Rune { rune, raw } => {
                let result = provider.get_rune(&rune).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Rune {}:
{}", rune, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::Sat { sat, raw } => {
                let result = provider.get_sat(sat).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Sat {}:
{}", sat, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
            OrdCommands::TxInfo { txid, raw } => {
                let result = provider.get_tx_info(&txid).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Transaction {}:
{}", txid, serde_json::to_string_pretty(&result)?);
                }
                Ok(())
            },
        };
        res.map_err(|e| AlkanesError::Wallet(e.to_string()))
    }
}

/// Expand tilde (~) in file paths to home directory
fn expand_tilde(path: &str) -> Result<String> {
    if path.starts_with("~/") {
        let home = std::env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(path.replacen("~", &home, 1))
    } else {
        Ok(path.to_string())
    }
}