//! Contract deployment and execution functionality

use crate::Result;
use anyhow::Context;
use log::info;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use alloc::sync::Arc;


use crate::{ToString, format};

#[cfg(not(target_arch = "wasm32"))]
use std::{vec::Vec, string::String};
#[cfg(target_arch = "wasm32")]
use alloc::{vec::Vec, string::String};

use crate::rpc::RpcClient;
use crate::wallet::WalletManager;
use super::types::*;

/// Contract operations manager
pub struct ContractManager<P: crate::traits::DeezelProvider> {
    rpc_client: Arc<RpcClient<P>>,
    _wallet_manager: Arc<WalletManager<P>>,
}

impl<P: crate::traits::DeezelProvider> ContractManager<P> {
    /// Create a new contract manager
    pub fn new(rpc_client: Arc<RpcClient<P>>, wallet_manager: Arc<WalletManager<P>>) -> Self {
        Self {
            rpc_client,
            _wallet_manager: wallet_manager,
        }
    }

    /// Deploy a new smart contract
    pub async fn deploy_contract(&self, params: ContractDeployParams) -> Result<ContractDeployResult> {
        info!("Deploying contract from WASM file: {}", params.wasm_file);
        
        // Read WASM file
        #[cfg(not(target_arch = "wasm32"))]
        let wasm_hex = {
            let wasm_bytes = std::fs::read(&params.wasm_file)
                .with_context(|| format!("Failed to read WASM file: {}", params.wasm_file))?;
            hex::encode(wasm_bytes)
        };
        
        #[cfg(target_arch = "wasm32")]
        let wasm_hex = {
            let _ = &params;
            return Err(crate::DeezelError::Validation("File system operations not supported in WASM environment".to_string()));
        };
        
        // Create deployment transaction
        let deploy_result = self.rpc_client.call(
            "http://localhost:8080",
            "deploy_contract",
            serde_json::json!({
                "wasm_bytecode": wasm_hex,
                "calldata": params.calldata
            })
        ).await?;
        
        // Parse the deployment result
        let block = deploy_result.get("block")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let tx = deploy_result.get("tx")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let txid = deploy_result.get("txid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let fee = deploy_result.get("fee")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);
        
        let contract_id = AlkaneId { block, tx };
        
        info!("Contract deployed successfully: {}:{}", contract_id.block, contract_id.tx);
        
        Ok(ContractDeployResult {
            contract_id,
            txid,
            fee,
        })
    }

    /// Execute a contract function
    pub async fn execute_contract(&self, params: ContractExecuteParams) -> Result<TransactionResult> {
        info!("Executing contract with calldata: {:?}", params.calldata);
        
        // Validate that we have a target contract
        if params.target.block == 0 && params.target.tx == 0 {
            return Err(crate::DeezelError::Validation("Invalid contract target".to_string()));
        }
        
        // Create execution transaction
        let execute_result = self.rpc_client.call(
            "http://localhost:8080",
            "execute_contract",
            serde_json::json!({
                "target": format!("{}:{}", params.target.block, params.target.tx),
                "calldata": params.calldata,
                "edicts": params.edicts.iter().map(|e| {
                    serde_json::json!({
                        "alkane_id": format!("{}:{}", e.alkane_id.block, e.alkane_id.tx),
                        "amount": e.amount,
                        "output": e.output
                    })
                }).collect::<Vec<_>>(),
                "tokens": params.tokens.iter().map(|t| {
                    serde_json::json!({
                        "alkane_id": format!("{}:{}", t.alkane_id.block, t.alkane_id.tx),
                        "amount": t.amount
                    })
                }).collect::<Vec<_>>()
            })
        ).await?;
        
        // Parse the execution result
        let txid = execute_result.get("txid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let fee = execute_result.get("fee")
            .and_then(|v| v.as_u64())
            .unwrap_or(500);
        
        info!("Contract execution completed: {}", txid);
        
        Ok(TransactionResult { txid, fee })
    }

    /// Get contract bytecode
    pub async fn get_bytecode(&self, contract_id: &AlkaneId) -> Result<String> {
        info!("Getting bytecode for contract: {}:{}", contract_id.block, contract_id.tx);
        
        self.rpc_client.get_bytecode(
            &contract_id.block.to_string(),
            &contract_id.tx.to_string()
        ).await
    }

    /// Get contract metadata
    pub async fn get_metadata(&self, contract_id: &AlkaneId) -> Result<serde_json::Value> {
        info!("Getting metadata for contract: {}:{}", contract_id.block, contract_id.tx);
        
        self.rpc_client.get_contract_meta(
            &contract_id.block.to_string(),
            &contract_id.tx.to_string()
        ).await
    }
}

/// Parse calldata from comma-separated string
pub fn parse_calldata(calldata_str: &str) -> Vec<String> {
    calldata_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse edicts from string format "block:tx:amount:output,block:tx:amount:output,..."
pub fn parse_edicts(edicts_str: &str) -> Result<Vec<Edict>> {
    let mut edicts = Vec::new();
    
    for edict_part in edicts_str.split(',') {
        let parts: Vec<&str> = edict_part.trim().split(':').collect();
        if parts.len() != 4 {
            return Err(crate::DeezelError::Parse("Invalid edict format. Expected 'block:tx:amount:output'".to_string()));
        }
        
        let block = parts[0].parse::<u64>()
            .context("Invalid block number in edict")?;
        let tx = parts[1].parse::<u64>()
            .context("Invalid transaction number in edict")?;
        let amount = parts[2].parse::<u64>()
            .context("Invalid amount in edict")?;
        let output = parts[3].parse::<u32>()
            .context("Invalid output index in edict")?;
        
        edicts.push(Edict {
            alkane_id: AlkaneId { block, tx },
            amount,
            output,
        });
    }
    
    Ok(edicts)
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::*;

    #[test]
    fn test_parse_calldata() {
        let calldata = parse_calldata("arg1,arg2,arg3");
        assert_eq!(calldata, vec!["arg1", "arg2", "arg3"]);
        
        let empty_calldata = parse_calldata("");
        assert_eq!(empty_calldata, Vec::<String>::new());
        
        let spaced_calldata = parse_calldata("arg1, arg2 , arg3");
        assert_eq!(spaced_calldata, vec!["arg1", "arg2", "arg3"]);
    }

    #[test]
    fn test_parse_edicts() {
        let edicts = parse_edicts("123:456:1000:0,789:012:2000:1").unwrap();
        assert_eq!(edicts.len(), 2);
        
        assert_eq!(edicts[0].alkane_id.block, 123);
        assert_eq!(edicts[0].alkane_id.tx, 456);
        assert_eq!(edicts[0].amount, 1000);
        assert_eq!(edicts[0].output, 0);
        
        assert_eq!(edicts[1].alkane_id.block, 789);
        assert_eq!(edicts[1].alkane_id.tx, 12);
        assert_eq!(edicts[1].amount, 2000);
        assert_eq!(edicts[1].output, 1);
    }

    #[test]
    fn test_parse_invalid_edicts() {
        assert!(parse_edicts("invalid").is_err());
        assert!(parse_edicts("123:456:1000").is_err());
        assert!(parse_edicts("123:456:1000:0:extra").is_err());
    }
}