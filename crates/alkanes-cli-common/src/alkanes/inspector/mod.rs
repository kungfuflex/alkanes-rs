//! Core alkanes inspector functionality for WASM-compatible environments
//!
//! This module provides the core business logic for alkanes inspection,
//! including fuzzing, metadata extraction, disassembly, and codehash computation.
//! It uses trait abstractions to be platform-agnostic and WASM-compatible.
//!
//! Enhanced with full WASM runtime integration and rich execution details
//! including host call interception, detailed error information, and comprehensive
//! execution context management.

pub mod types;
pub mod runtime;
pub mod analysis;

use anyhow::{Context, Result};
use crate::traits::AlkanesProvider;
use crate::alkanes::types::AlkaneId;
pub use types::{
    AlkaneMetadata, AlkaneMethod, AlkanesRuntimeContext, AlkanesState, ExecutionResult,
    FuzzingResults, HostCall, InspectionConfig, InspectionResult, MessageContextParcel,
};

#[cfg(not(feature = "std"))]
use alloc::string::ToString;
#[cfg(feature = "std")]
use std::string::ToString;

/// Core alkanes inspector that works with trait abstractions
#[cfg(feature = "wasm-inspection")]
pub struct AlkaneInspector<P: AlkanesProvider> {
    rpc_provider: P,
}

#[cfg(feature = "wasm-inspection")]
impl<P: AlkanesProvider> AlkaneInspector<P> {
    /// Create a new alkane inspector
    pub fn new(rpc_provider: P) -> Self {
        Self { rpc_provider }
    }

    /// Inspect an alkane with the specified configuration
    pub async fn inspect_alkane(
        &self,
        alkane_id: &AlkaneId,
        config: &InspectionConfig,
    ) -> Result<InspectionResult> {
        // Get the WASM bytecode for the alkane
        let bytecode = self.get_alkane_bytecode(alkane_id).await?;
        
        // Remove 0x prefix if present
        let hex_string = bytecode.strip_prefix("0x").unwrap_or(&bytecode);
        
        let wasm_bytes = hex::decode(hex_string)
            .with_context(|| "Failed to decode WASM bytecode from hex".to_string())?;
        
        let mut result = InspectionResult {
            alkane_id: alkane_id.clone(),
            bytecode_length: wasm_bytes.len(),
            codehash: None,
            disassembly: None,
            metadata: None,
            metadata_error: None,
            fuzzing_results: None,
        };
        
        // Perform requested analysis
        if config.codehash {
            result.codehash = Some(analysis::compute_codehash(&wasm_bytes)?);
        }
        
        if config.meta {
            match analysis::extract_metadata(&wasm_bytes).await {
                Ok(meta) => result.metadata = Some(meta),
                Err(e) => result.metadata_error = Some(e.to_string()),
            }
        }
        
        if config.disasm {
            result.disassembly = analysis::disassemble_wasm(&wasm_bytes)?;
        }
        
        if config.fuzz {
            result.fuzzing_results = Some(analysis::perform_fuzzing_analysis(
                alkane_id, 
                &wasm_bytes, 
                config.fuzz_ranges.as_deref()
            ).await?);
        }
        
        Ok(result)
    }

    /// Get WASM bytecode for an alkane
    async fn get_alkane_bytecode(&self, alkane_id: &AlkaneId) -> Result<String> {
        self.rpc_provider.get_bytecode(&alkane_id.to_string(), None).await
        .map_err(|e| anyhow::anyhow!("Failed to get bytecode: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::AlkanesProvider;
    use crate::AlkanesError;
    use async_trait::async_trait;
    use crate::alkanes::types::{
        AlkaneId, EnhancedExecuteParams, EnhancedExecuteResult, ExecutionState, ReadyToSignCommitTx,
        ReadyToSignRevealTx, ReadyToSignTx,
    };
    use crate::alkanes::{AlkaneBalance, AlkanesInspectConfig, AlkanesInspectResult};
    use crate::alkanes::protorunes::{ProtoruneOutpointResponse, ProtoruneWalletResponse};
    use crate::proto::alkanes as alkanes_pb;
    use serde_json::Value as JsonValue;
    use super::types::InspectionConfig;

    struct MockRpcProvider;

    #[async_trait(?Send)]
    impl AlkanesProvider for MockRpcProvider {
        async fn execute(&mut self, _params: EnhancedExecuteParams) -> Result<ExecutionState, AlkanesError> {
            unimplemented!()
        }

        async fn resume_execution(
            &mut self,
            _state: ReadyToSignTx,
            _params: &EnhancedExecuteParams,
        ) -> Result<EnhancedExecuteResult, AlkanesError> {
            unimplemented!()
        }

        async fn resume_commit_execution(
            &mut self,
            _state: ReadyToSignCommitTx,
        ) -> Result<ExecutionState, AlkanesError> {
            unimplemented!()
        }

        async fn resume_reveal_execution(
            &mut self,
            _state: ReadyToSignRevealTx,
        ) -> Result<EnhancedExecuteResult, AlkanesError> {
            unimplemented!()
        }
        
        async fn protorunes_by_address(
            &self,
            _address: &str,
            _block_tag: Option<String>,
            _protocol_tag: u128,
        ) -> Result<ProtoruneWalletResponse, AlkanesError> {
            unimplemented!()
        }
        async fn protorunes_by_outpoint(
            &self,
            _txid: &str,
            _vout: u32,
            _block_tag: Option<String>,
            _protocol_tag: u128,
        ) -> Result<ProtoruneOutpointResponse, AlkanesError> {
            unimplemented!()
        }
        async fn view(&self, _contract_id: &str, _view_fn: &str, _params: Option<&[u8]>) -> Result<JsonValue, AlkanesError> {
            unimplemented!()
        }
        async fn trace(&self, _outpoint: &str) -> Result<alkanes_pb::Trace, AlkanesError> {
            unimplemented!()
        }
        async fn get_block(&self, _height: u64) -> Result<alkanes_pb::BlockResponse, AlkanesError> {
            unimplemented!()
        }
        async fn sequence(&self) -> Result<JsonValue, AlkanesError> {
            unimplemented!()
        }
        async fn spendables_by_address(&self, _address: &str) -> Result<JsonValue, AlkanesError> {
            unimplemented!()
        }
        async fn trace_block(&self, _height: u64) -> Result<alkanes_pb::Trace, AlkanesError> {
            unimplemented!()
        }
        async fn get_bytecode(&self, _alkane_id: &str, _block_tag: Option<String>) -> Result<String, AlkanesError> {
            Ok("0x".to_string())
        }
        async fn inspect(&self, _target: &str, _config: AlkanesInspectConfig) -> Result<AlkanesInspectResult, AlkanesError> {
            unimplemented!()
        }
        async fn get_balance(&self, _address: Option<&str>) -> Result<Vec<AlkaneBalance>, AlkanesError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_alkane_inspector_creation() {
        let provider = MockRpcProvider;
        let inspector = AlkaneInspector::new(provider);
        
        let alkane_id = AlkaneId { block: 1, tx: 100 };
        let config = InspectionConfig {
            disasm: false,
            fuzz: false,
            fuzz_ranges: None,
            meta: false,
            codehash: true,
            raw: false,
        };
        
        let result = inspector.inspect_alkane(&alkane_id, &config).await;
        assert!(result.is_ok());
    }
}