//! Esplora, Runestone, Alkanes, and Monitor provider implementations for WebProvider
//
// This module contains the remaining trait implementations for WebProvider
// that couldn't fit in the main provider.rs file due to size constraints.

use async_trait::async_trait;
use bitcoin::{
    secp256k1::{schnorr::Signature, All, Secp256k1, Message},
    OutPoint, TxOut,
};
use alkanes_cli_common::{*, alkanes::{AlkanesInspectConfig, AlkanesInspectResult, AlkaneBalance}};
use serde_json::Value as JsonValue;

#[cfg(target_arch = "wasm32")]
use alloc::{
    vec::Vec,
    boxed::Box,
    string::{String, ToString},
};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    vec::Vec,
    boxed::Box,
    string::String,
};

use crate::provider::WebProvider;

// EsploraProvider implementation is now in provider.rs
// RunestoneProvider implementation
#[async_trait(?Send)]
impl RunestoneProvider for WebProvider {
    async fn decode_runestone(&self, tx: &bitcoin::Transaction) -> Result<JsonValue> {
        let tx_hex = bitcoin::consensus::encode::serialize_hex(tx);
        self.call(self.sandshrew_rpc_url(), "runestone_decode", serde_json::json!([tx_hex]), 1).await
    }

    async fn format_runestone_with_decoded_messages(&self, tx: &bitcoin::Transaction) -> Result<JsonValue> {
        let tx_hex = bitcoin::consensus::encode::serialize_hex(tx);
        self.call(self.sandshrew_rpc_url(), "runestone_format", serde_json::json!([tx_hex]), 1).await
    }

    async fn analyze_runestone(&self, txid: &str) -> Result<JsonValue> {
        self.call(self.sandshrew_rpc_url(), "runestone_analyze", serde_json::json!([txid]), 1).await
    }
}
// AlkanesProvider implementation
#[async_trait(?Send)]
impl AlkanesProvider for WebProvider {
    async fn execute(&mut self, params: alkanes_cli_common::alkanes::types::EnhancedExecuteParams) -> Result<alkanes_cli_common::alkanes::types::ExecutionState> {
        let result = self.call(self.sandshrew_rpc_url(), "alkanes_execute", serde_json::to_value(params)?, 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }

    async fn resume_execution(
        &mut self,
        _state: alkanes_cli_common::alkanes::types::ReadyToSignTx,
        _params: &alkanes_cli_common::alkanes::types::EnhancedExecuteParams,
    ) -> Result<alkanes_cli_common::alkanes::types::EnhancedExecuteResult> {
        unimplemented!("resume_execution is not implemented for WebProvider")
    }

    async fn resume_commit_execution(
        &mut self,
        _state: alkanes_cli_common::alkanes::types::ReadyToSignCommitTx,
    ) -> Result<alkanes_cli_common::alkanes::types::ExecutionState> {
        unimplemented!("resume_commit_execution is not implemented for WebProvider")
    }

    async fn resume_reveal_execution(
        &mut self,
        _state: alkanes_cli_common::alkanes::types::ReadyToSignRevealTx,
    ) -> Result<alkanes_cli_common::alkanes::types::EnhancedExecuteResult> {
        unimplemented!("resume_reveal_execution is not implemented for WebProvider")
    }

    async fn protorunes_by_address(&self, _address: &str, _block_tag: Option<String>, _protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneWalletResponse> {
        unimplemented!()
    }

    async fn protorunes_by_outpoint(&self, _txid: &str, _vout: u32, _block_tag: Option<String>, _protocol_tag: u128) -> Result<alkanes_cli_common::alkanes::protorunes::ProtoruneOutpointResponse> {
        unimplemented!()
    }

    async fn simulate(&self, _contract_id: &str, _params: Option<&str>) -> Result<JsonValue> {
        unimplemented!()
    }

    async fn trace(&self, _outpoint: &str) -> Result<alkanes_support::proto::alkanes::Trace> {
        unimplemented!()
    }

    async fn get_block(&self, _height: u64) -> Result<alkanes_support::proto::alkanes::BlockResponse> {
        unimplemented!()
    }

    async fn sequence(&self, _txid: &str, _vout: u32) -> Result<JsonValue> {
        unimplemented!()
    }

    async fn spendables_by_address(&self, _address: &str) -> Result<JsonValue> {
        unimplemented!()
    }

    async fn trace_block(&self, _height: u64) -> Result<alkanes_support::proto::alkanes::Trace> {
        unimplemented!()
    }

    async fn get_bytecode(&self, _alkane_id: &str) -> Result<String> {
        unimplemented!()
    }

    async fn inspect(&self, _target: &str, _config: AlkanesInspectConfig) -> Result<AlkanesInspectResult> {
        unimplemented!()
    }

    async fn get_balance(&self, _address: Option<&str>) -> Result<Vec<AlkaneBalance>> {
        unimplemented!()
    }
}
// MonitorProvider implementation
#[async_trait(?Send)]
impl MonitorProvider for WebProvider {
    async fn monitor_blocks(&self, start: Option<u64>) -> Result<()> {
        let params = if let Some(s) = start {
            serde_json::json!([s])
        } else {
            serde_json::json!([])
        };
        self.call(self.sandshrew_rpc_url(), "monitor_blocks", params, 1).await?;
        Ok(())
    }

    async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
        let result = self.call(self.sandshrew_rpc_url(), "monitor_events", serde_json::json!([height]), 1).await?;
        serde_json::from_value(result).map_err(|e| AlkanesError::Serialization(e.to_string()))
    }
}
// OrdProvider implementation
// OrdProvider implementation is now in provider.rs

#[async_trait(?Send)]
impl MetashrewProvider for WebProvider {
    async fn get_height(&self) -> Result<u64> {
        Err(AlkanesError::NotImplemented("Metashrew operations not implemented for web provider".to_string()))
    }
    async fn get_block_hash(&self, _height: u64) -> Result<String> {
        Err(AlkanesError::NotImplemented("Metashrew operations not implemented for web provider".to_string()))
    }
    async fn get_state_root(&self, _height: JsonValue) -> Result<String> {
        Err(AlkanesError::NotImplemented("Metashrew operations not implemented for web provider".to_string()))
    }
}

// DeezelProvider implementation
#[async_trait(?Send)]
impl KeystoreProvider for WebProvider {
    async fn derive_addresses(&self, _master_public_key: &str, _network: Network, _script_types: &[&str], _start_index: u32, _count: u32) -> Result<Vec<KeystoreAddress>> {
        Err(AlkanesError::NotImplemented("Keystore operations not implemented for web provider".to_string()))
    }
    
    async fn get_default_addresses(&self, _master_public_key: &str, _network: Network) -> Result<Vec<KeystoreAddress>> {
        Err(AlkanesError::NotImplemented("Keystore operations not implemented for web provider".to_string()))
    }
    
    fn parse_address_range(&self, _range_spec: &str) -> Result<(String, u32, u32)> {
        Err(AlkanesError::NotImplemented("Keystore operations not implemented for web provider".to_string()))
    }
    
    async fn get_keystore_info(&self, _master_fingerprint: &str, _created_at: u64, _version: &str) -> Result<KeystoreInfo> {
        Err(AlkanesError::NotImplemented("Keystore operations not implemented for web provider".to_string()))
    }
    async fn get_address(&self, _address_type: &str, _index: u32) -> Result<String> {
        Err(AlkanesError::NotImplemented("Keystore operations not implemented for web provider".to_string()))
    }
}

#[async_trait(?Send)]
impl DeezelProvider for WebProvider {
    fn provider_name(&self) -> &str {
        "WebProvider"
    }

    async fn initialize(&self) -> Result<()> {
        // No-op for web provider
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // No-op for web provider
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn DeezelProvider> {
        Box::new(self.clone())
    }

    fn secp(&self) -> &Secp256k1<All> {
        todo!()
    }

    async fn get_utxo(&self, _outpoint: &OutPoint) -> Result<Option<TxOut>> {
        todo!()
    }

    async fn sign_taproot_script_spend(&self, _msg: Message) -> Result<Signature> {
        todo!()
    }
    fn get_bitcoin_rpc_url(&self) -> Option<String> {
        Some(self.sandshrew_rpc_url().to_string())
    }
    fn get_esplora_api_url(&self) -> Option<String> {
        self.esplora_rpc_url().map(|s| s.to_string())
    }
    fn get_ord_server_url(&self) -> Option<String> {
        Some(self.sandshrew_rpc_url().to_string())
    }
}