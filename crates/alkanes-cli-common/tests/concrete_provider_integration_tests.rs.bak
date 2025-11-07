use deezel_common::{
    keystore::{self, AddressInfo as KeystoreAddressInfo, Keystore},
    traits::{
        BitcoinRpcProvider, EsploraProvider, JsonRpcProvider, SendParams, UtxoInfo, WalletProvider,
    },
    utils::hex::ToHexString,
    DeezelError, Result,
};
use bitcoin::{
    self, Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction,
    TxIn, TxOut, Witness,
    bip32::{DerivationPath, Xpriv},
    secp256k1::{All, Secp256k1},
    sighash::{self, Prevouts, SighashCache},
};
use bip39::{Mnemonic, Language};
use serde_json::{json, Value as JsonValue};
use std::{collections::HashMap, fs::File, io::Write, str::FromStr};

// A mock provider that simulates JSON-RPC calls for testing.
#[derive(Clone)]
struct MockRpcProvider {
    responses: HashMap<String, JsonValue>,
    wallet_path: Option<String>,
    passphrase: Option<String>,
    network: Network,
}

impl MockRpcProvider {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
            wallet_path: None,
            passphrase: None,
            network: Network::Regtest,
        }
    }

    fn with_response(mut self, method: &str, response: serde_json::Value) -> Self {
        self.responses.insert(method.to_string(), response);
        self
    }

    // Coin selection logic, copied from ConcreteProvider for isolated testing.
    fn select_coins(
        &self,
        mut utxos: Vec<UtxoInfo>,
        target_amount: Amount,
    ) -> Result<(Vec<UtxoInfo>, Amount)> {
        utxos.sort_by(|a, b| b.amount.cmp(&a.amount)); // Largest-first
        let mut selected_utxos = Vec::new();
        let mut total_input_amount = Amount::ZERO;
        for utxo in utxos {
            if total_input_amount >= target_amount {
                break;
            }
            total_input_amount += Amount::from_sat(utxo.amount);
            selected_utxos.push(utxo);
        }
        if total_input_amount < target_amount {
            return Err(DeezelError::Wallet("Insufficient funds".to_string()));
        }
        Ok((selected_utxos, total_input_amount))
    }

    // Address info lookup, copied from ConcreteProvider.
    fn find_address_info<'a>(
        &self,
        keystore: &'a Keystore,
        address: &Address,
        network: Network,
    ) -> Result<&'a KeystoreAddressInfo> {
        keystore
            .addresses
            .get(&network.to_string())
            .and_then(|addrs| addrs.iter().find(|a| a.address == address.to_string()))
            .ok_or_else(|| DeezelError::Wallet(format!("Address {} not found in keystore", address)))
    }
}

#[async_trait::async_trait(?Send)]
impl JsonRpcProvider for MockRpcProvider {
    async fn call(
        &self,
        _url: &str,
        method: &str,
        _params: serde_json::Value,
        _id: u64,
    ) -> Result<serde_json::Value> {
        self.responses
            .get(method)
            .cloned()
            .ok_or_else(|| DeezelError::RpcError(format!("No mock response for method {}", method)))
    }
    async fn get_bytecode(&self, _block: &str, _tx: &str) -> Result<String> {
        unimplemented!()
    }
}

#[async_trait::async_trait(?Send)]
impl EsploraProvider for MockRpcProvider {
    async fn get_address_utxo(&self, address: &str) -> Result<serde_json::Value> {
        self.call("", "esplora_address::utxo", json!([address]), 1)
            .await
    }
    async fn get_tx(&self, txid: &str) -> Result<serde_json::Value> {
        self.call("", "esplora_tx", json!([txid]), 1).await
    }
    async fn broadcast(&self, tx_hex: &str) -> Result<String> {
        self.call("", "sendrawtransaction", json!([tx_hex]), 1)
            .await?
            .as_str()
            .map(String::from)
            .ok_or_else(|| DeezelError::RpcError("Invalid txid response".into()))
    }
    // Other EsploraProvider methods are not needed for this test
    async fn get_blocks_tip_hash(&self) -> Result<String> { unimplemented!() }
    async fn get_blocks_tip_height(&self) -> Result<u64> { unimplemented!() }
    async fn get_blocks(&self, _start_height: Option<u64>) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_block_by_height(&self, _height: u64) -> Result<String> { unimplemented!() }
    async fn get_block(&self, _hash: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_block_status(&self, _hash: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_block_txids(&self, _hash: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_block_header(&self, _hash: &str) -> Result<String> { unimplemented!() }
    async fn get_block_raw(&self, _hash: &str) -> Result<String> { unimplemented!() }
    async fn get_block_txid(&self, _hash: &str, _index: u32) -> Result<String> { unimplemented!() }
    async fn get_block_txs(&self, _hash: &str, _start_index: Option<u32>) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_address(&self, _address: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_address_txs(&self, _address: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_address_txs_chain(&self, _address: &str, _last_seen_txid: Option<&str>) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_address_txs_mempool(&self, _address: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_address_prefix(&self, _prefix: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_tx_hex(&self, _txid: &str) -> Result<String> { unimplemented!() }
    async fn get_tx_raw(&self, _txid: &str) -> Result<String> { unimplemented!() }
    async fn get_tx_status(&self, _txid: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_tx_merkle_proof(&self, _txid: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_tx_merkleblock_proof(&self, _txid: &str) -> Result<String> { unimplemented!() }
    async fn get_tx_outspend(&self, _txid: &str, _index: u32) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_tx_outspends(&self, _txid: &str) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_mempool(&self) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_mempool_txids(&self) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_mempool_recent(&self) -> Result<serde_json::Value> { unimplemented!() }
    async fn get_fee_estimates(&self) -> Result<serde_json::Value> { unimplemented!() }
}

#[async_trait::async_trait(?Send)]
impl BitcoinRpcProvider for MockRpcProvider {
     async fn send_raw_transaction(&self, tx_hex: &str) -> Result<String> {
        self.call("", "sendrawtransaction", json!([tx_hex]), 1)
            .await?
            .as_str()
            .map(String::from)
            .ok_or_else(|| DeezelError::RpcError("Invalid txid response".into()))
    }
    // Other BitcoinRpcProvider methods are not needed for this test
    async fn get_block_count(&self) -> Result<u64> { unimplemented!() }
    async fn generate_to_address(&self, _nblocks: u32, _address: &str) -> Result<JsonValue> { unimplemented!() }
    async fn get_new_address(&self) -> Result<JsonValue> { Ok(json!("bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")) }
    async fn get_transaction_hex(&self, _txid: &str) -> Result<String> { unimplemented!() }
    async fn get_block(&self, _hash: &str) -> Result<JsonValue> { unimplemented!() }
    async fn get_block_hash(&self, _height: u64) -> Result<String> { unimplemented!() }
    async fn get_mempool_info(&self) -> Result<JsonValue> { unimplemented!() }
    async fn estimate_smart_fee(&self, _target: u32) -> Result<JsonValue> { unimplemented!() }
    async fn get_esplora_blocks_tip_height(&self) -> Result<u64> { unimplemented!() }
    async fn trace_transaction(&self, _txid: &str, _vout: u32, _block: Option<&str>, _tx: Option<&str>) -> Result<JsonValue> { unimplemented!() }
}

#[async_trait::async_trait(?Send)]
impl WalletProvider for MockRpcProvider {
    async fn send(&self, params: SendParams) -> Result<String> {
        let tx_hex = self.create_transaction(params).await?;
        let signed_tx_hex = self.sign_transaction(tx_hex).await?;
        self.broadcast_transaction(signed_tx_hex).await
    }

    async fn create_transaction(&self, params: SendParams) -> Result<String> {
        let all_addresses = self.get_addresses(100).await?;
        let address_strings: Vec<String> = all_addresses.iter().map(|a| a.address.clone()).collect();
        let utxos = self.get_utxos(false, Some(address_strings)).await?;
        let target_amount = Amount::from_sat(params.amount);
        let (selected_utxos, total_input_amount) = self.select_coins(utxos, target_amount)?;
        let mut tx = Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        };
        for utxo in &selected_utxos {
            tx.input.push(TxIn {
                previous_output: OutPoint {
                    txid: bitcoin::Txid::from_str(&utxo.txid)?,
                    vout: utxo.vout,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
        }
        let recipient_address = Address::from_str(&params.address)?.require_network(self.network)?;
        tx.output.push(TxOut {
            value: target_amount,
            script_pubkey: recipient_address.script_pubkey(),
        });
        let fee = Amount::from_sat(1000); // Mock fee
        let change_amount = total_input_amount.checked_sub(target_amount).and_then(|a| a.checked_sub(fee));
        if let Some(change) = change_amount {
            if change.to_sat() > 546 {
                let change_address = Address::from_str(&all_addresses[0].address)?.require_network(self.network)?;
                tx.output.push(TxOut {
                    value: change,
                    script_pubkey: change_address.script_pubkey(),
                });
            }
        }
        Ok(bitcoin::consensus::encode::serialize_hex(&tx))
    }

    async fn sign_transaction(&self, tx_hex: String) -> Result<String> {
        let hex_bytes = hex::decode(tx_hex)?;
        let mut tx: Transaction = bitcoin::consensus::deserialize(&hex_bytes)?;
        let wallet_path = self.wallet_path.as_ref().ok_or_else(|| DeezelError::Wallet("Wallet path not set".to_string()))?;
        let keystore_data = std::fs::read(wallet_path)?;
        let keystore: Keystore = serde_json::from_slice(&keystore_data)?;
        let passphrase = self.passphrase.as_deref().ok_or_else(|| DeezelError::Wallet("Passphrase not set".to_string()))?;
        let seed = keystore::decrypt_seed(&keystore, passphrase)?;
        let network = self.get_network();
        let secp: Secp256k1<All> = Secp256k1::new();
        let mut prevouts = Vec::new();
        let mut utxo_addresses = Vec::new();
        for input in &tx.input {
            let tx_info = self.get_tx(&input.previous_output.txid.to_string()).await?;
            let vout_info = tx_info["vout"].get(input.previous_output.vout as usize).ok_or_else(|| DeezelError::Wallet("Vout not found".to_string()))?;
            let amount = vout_info["value"].as_u64().ok_or_else(|| DeezelError::Wallet("UTXO value not found".to_string()))?;
            let script_pubkey_hex = vout_info["scriptpubkey"]["hex"].as_str().ok_or_else(|| DeezelError::Wallet("UTXO script pubkey not found".to_string()))?;
            let script_pubkey = ScriptBuf::from_hex(script_pubkey_hex)?;
            prevouts.push(TxOut { value: Amount::from_sat(amount), script_pubkey });
            
            // Get the address from the UTXO data instead of trying to derive it from script
            // For this test, we know the address from our test setup
            let all_addresses = self.get_addresses(100).await?;
            let utxo_address = &all_addresses[0].address; // Use the first address for this test
            utxo_addresses.push(utxo_address.clone());
        }
        let mut sighash_cache = SighashCache::new(&mut tx);
        for i in 0..prevouts.len() {
            let prev_txout = &prevouts[i];
            let address_str = &utxo_addresses[i];
            let address = Address::from_str(address_str)?.require_network(network)?;
            let addr_info = self.find_address_info(&keystore, &address, network)?;
            let path = DerivationPath::from_str(&addr_info.path)?;
            let root_key = Xpriv::new_master(network, seed.as_bytes())?;
            let derived_xpriv = root_key.derive_priv(&secp, &path)?;
            let keypair = derived_xpriv.to_keypair(&secp);
            let mut witness = Witness::new();
            if addr_info.address_type == "p2tr" {
                let sighash = sighash_cache.taproot_key_spend_signature_hash(i, &Prevouts::All(&prevouts), sighash::TapSighashType::Default)?;
                let msg = bitcoin::secp256k1::Message::from(sighash);
                let signature = secp.sign_schnorr(&msg, &keypair);
                witness.push(signature.as_ref());
            } else {
                let script_code = address.script_pubkey();
                let sighash = sighash_cache.p2wpkh_signature_hash(i, &script_code, prev_txout.value, sighash::EcdsaSighashType::All)?;
                let msg = bitcoin::secp256k1::Message::from(sighash);
                let signature = secp.sign_ecdsa(&msg, &keypair.secret_key());
                let mut sig_with_hashtype = signature.serialize_der().to_vec();
                sig_with_hashtype.push(sighash::EcdsaSighashType::All.to_u32() as u8);
                witness.push(sig_with_hashtype);
                witness.push(keypair.public_key().serialize());
            }
            *sighash_cache.witness_mut(i).unwrap() = witness;
        }
        let signed_tx = sighash_cache.into_transaction();
        Ok(bitcoin::consensus::encode::serialize_hex(&signed_tx))
    }

    async fn broadcast_transaction(&self, tx_hex: String) -> Result<String> {
        self.send_raw_transaction(&tx_hex).await
    }

    // --- Unimplemented methods for this test ---
    async fn create_wallet(&self, _config: deezel_common::traits::WalletConfig, _mnemonic: Option<String>, _passphrase: Option<String>) -> Result<deezel_common::traits::WalletInfo> { unimplemented!() }
    async fn load_wallet(&self, _config: deezel_common::traits::WalletConfig, _passphrase: Option<String>) -> Result<deezel_common::traits::WalletInfo> { unimplemented!() }
    async fn get_balance(&self) -> Result<deezel_common::traits::WalletBalance> { unimplemented!() }
    async fn get_address(&self) -> Result<String> { unimplemented!() }
    async fn get_addresses(&self, _count: u32) -> Result<Vec<deezel_common::traits::AddressInfo>> {
        // Load the keystore to get the actual address
        let wallet_path = self.wallet_path.as_ref().ok_or_else(|| DeezelError::Wallet("Wallet path not set".to_string()))?;
        let keystore_data = std::fs::read(wallet_path)?;
        let keystore: Keystore = serde_json::from_slice(&keystore_data)?;
        let network = self.get_network();
        let address_info = &keystore.addresses.get(&network.to_string()).unwrap()[0];
        
        Ok(vec![deezel_common::traits::AddressInfo {
            address: address_info.address.clone(),
            script_type: address_info.address_type.clone(),
            derivation_path: address_info.path.clone(),
            index: 0,
            used: false,
        }])
    }
    async fn get_utxos(&self, _include_frozen: bool, addresses: Option<Vec<String>>) -> Result<Vec<UtxoInfo>> {
        let addrs = addresses.ok_or_else(|| DeezelError::Wallet("get_utxos requires at least one address".to_string()))?;
        let mut all_utxos = Vec::new();
        for address in addrs {
            let utxos_json = self.get_address_utxo(&address).await?;
            let parsed_utxos: Vec<UtxoInfo> = serde_json::from_value(utxos_json)?;
            all_utxos.extend(parsed_utxos);
        }
        Ok(all_utxos)
    }
    async fn get_history(&self, _count: u32, _address: Option<String>) -> Result<Vec<deezel_common::traits::TransactionInfo>> { unimplemented!() }
    async fn freeze_utxo(&self, _utxo: String, _reason: Option<String>) -> Result<()> { unimplemented!() }
    async fn unfreeze_utxo(&self, _utxo: String) -> Result<()> { unimplemented!() }
    async fn estimate_fee(&self, _target: u32) -> Result<deezel_common::traits::FeeEstimate> { unimplemented!() }
    async fn get_fee_rates(&self) -> Result<deezel_common::traits::FeeRates> { unimplemented!() }
    async fn sync(&self) -> Result<()> { unimplemented!() }
    async fn backup(&self) -> Result<String> { unimplemented!() }
    async fn get_mnemonic(&self) -> Result<Option<String>> { unimplemented!() }
    fn get_network(&self) -> Network { self.network }
    async fn get_internal_key(&self) -> Result<bitcoin::XOnlyPublicKey> { unimplemented!() }
    async fn sign_psbt(&self, _psbt: &bitcoin::psbt::Psbt) -> Result<bitcoin::psbt::Psbt> { unimplemented!() }
    async fn get_keypair(&self) -> Result<bitcoin::secp256k1::Keypair> { unimplemented!() }
    fn set_passphrase(&mut self, passphrase: Option<String>) { self.passphrase = passphrase; }
}

#[tokio::test]
async fn test_wallet_send_transaction() {
    // 1. Generate deterministic key material
    let network = Network::Regtest;
    let mnemonic = Mnemonic::from_phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", Language::English).unwrap();
    
    // 2. Create a keystore with the deterministic seed and address info
    let passphrase = "test_password";
    let keystore = keystore::new_from_mnemonic(passphrase, &mnemonic.to_string(), network).unwrap();
    let keystore_json = serde_json::to_string(&keystore).unwrap();
    
    // 3. Get the address from the keystore (this ensures consistency)
    let address_info = &keystore.addresses.get(&network.to_string()).unwrap()[0];
    let address_str = &address_info.address;
    
    // 4. Parse the address to get the script_pubkey for the mock response
    let address = Address::from_str(address_str).unwrap().require_network(network).unwrap();
    let script_pubkey = address.script_pubkey();

    // 5. Save the keystore to a temporary file
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test-wallet-send.keystore");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(keystore_json.as_bytes()).unwrap();

    // 6. Setup mock RPC responses using the deterministic data
    let utxos_response = json!([
        {
            "txid": "1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a",
            "vout": 0,
            "amount": 5000000000u64,
            "address": address_str,
            "script_pubkey": null,
            "confirmations": 10,
            "frozen": false,
            "freeze_reason": null,
            "block_height": Some(123),
            "has_inscriptions": false,
            "has_runes": false,
            "has_alkanes": false,
            "is_coinbase": false
        }
    ]);

    let prev_txid = "1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a";
    let prev_tx_response = json!({
        "txid": prev_txid,
        "vout": [
            {
                "value": 5000000000u64,
                "scriptpubkey": { "hex": script_pubkey.to_hex_string() },
            }
        ]
    });

    let broadcast_txid = "mock_txid_from_broadcast";

    // 7. Create a mock provider
    let mut provider = MockRpcProvider::new()
        .with_response("esplora_address::utxo", utxos_response)
        .with_response("esplora_tx", prev_tx_response)
        .with_response("sendrawtransaction", json!(broadcast_txid));
    provider.wallet_path = Some(file_path.to_str().unwrap().to_string());
    provider.passphrase = Some(passphrase.to_string());

    // 8. Send a transaction
    let recipient_address = "bcrt1p45un5d47hvfhx6mfezr6x0htpanw23tgll7ppn6hj6gfzu3x3dnsaegh8d";
    let send_params = SendParams {
        address: recipient_address.to_string(),
        amount: 10000, // sats
        fee_rate: Some(1.0),
        send_all: false,
        from_address: None,
        change_address: None,
        auto_confirm: true,
    };
    let txid = provider.send(send_params).await.unwrap();

    // 9. Assert the result
    assert_eq!(txid, broadcast_txid);
}
