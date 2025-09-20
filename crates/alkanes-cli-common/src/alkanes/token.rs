//! Token operations functionality

use crate::Result;
use anyhow::Context;
use log::{debug, info};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use alloc::sync::Arc;


use crate::{ToString, format};

#[cfg(not(target_arch = "wasm32"))]
use std::{vec, vec::Vec};
#[cfg(target_arch = "wasm32")]
use alloc::{vec, vec::Vec};

use crate::rpc::RpcClient;
use crate::wallet::WalletManager;
use super::types::*;

/// Token operations manager
pub struct TokenManager<P: crate::traits::DeezelProvider> {
    rpc_client: Arc<RpcClient<P>>,
    _wallet_manager: Arc<WalletManager<P>>,
}

impl<P: crate::traits::DeezelProvider> TokenManager<P> {
    /// Create a new token manager
    pub fn new(rpc_client: Arc<RpcClient<P>>, wallet_manager: Arc<WalletManager<P>>) -> Self {
        Self {
            rpc_client,
            _wallet_manager: wallet_manager,
        }
    }

    /// Deploy a new alkanes token
    pub async fn deploy_token(&self, params: TokenDeployParams) -> Result<TokenDeployResult> {
        info!("Deploying token: {} ({})", params.name, params.symbol);
        debug!("Token parameters: cap={}, amount_per_mint={}, reserve_number={}",
               params.cap, params.amount_per_mint, params.reserve_number);
        
        // Create contract deployment parameters
        let _contract_params = super::types::ContractDeployParams {
            wasm_file: "token_contract.wasm".to_string(), // This would be the standard token contract
            calldata: vec![
                params.name.clone(),
                params.symbol.clone(),
                params.cap.to_string(),
                params.amount_per_mint.to_string(),
                params.reserve_number.to_string(),
            ],
            tokens: vec![], // No tokens needed for deployment
            fee_rate: params.fee_rate,
        };
        
        // Deploy the token contract
        let deploy_result = self.rpc_client.call(
            "http://localhost:8080", // Use configured endpoint
            "deploy_token_contract",
            serde_json::json!({
                "name": params.name,
                "symbol": params.symbol,
                "cap": params.cap,
                "amount_per_mint": params.amount_per_mint,
                "reserve_number": params.reserve_number,
                "premine": params.premine
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
            .unwrap_or(2000);
        
        let token_id = AlkaneId { block, tx };
        
        Ok(TokenDeployResult {
            token_id,
            txid,
            fee,
        })
    }

    /// Send alkanes tokens
    pub async fn send_token(&self, params: TokenSendParams) -> Result<TransactionResult> {
        info!("Sending {} units of token {}:{} to {}",
              params.amount, params.token.block, params.token.tx, params.to);
        
        // Validate recipient address
        if params.to.is_empty() {
            return Err(crate::DeezelError::Validation("Recipient address cannot be empty".to_string()));
        }
        
        // Validate amount
        if params.amount == 0 {
            return Err(crate::DeezelError::Validation("Amount must be greater than zero".to_string()));
        }
        
        // Check token balance first
        let from_address = params.from.as_deref().unwrap_or("default_address"); // Would get from wallet
        let balance = self.get_token_balance(&params.token, from_address).await?;
        
        if balance < params.amount {
            return Err(crate::DeezelError::Validation(
                format!("Insufficient balance: {} < {}", balance, params.amount)
            ));
        }
        
        // Create token transfer transaction
        let transfer_result = self.rpc_client.call(
            "http://localhost:8080",
            "transfer_token",
            serde_json::json!({
                "token_id": format!("{}:{}", params.token.block, params.token.tx),
                "from": from_address,
                "to": params.to,
                "amount": params.amount
            })
        ).await?;
        
        // Parse the transfer result
        let txid = transfer_result.get("txid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let fee = transfer_result.get("fee")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);
        
        Ok(TransactionResult { txid, fee })
    }

    /// Get token information
    pub async fn get_token_info(&self, token_id: &AlkaneId) -> Result<TokenInfo> {
        info!("Getting token info for: {}:{}", token_id.block, token_id.tx);
        
        // Use the trace method to get token information
        let trace_result = self.rpc_client.trace_transaction(
            &format!("{}:{}", token_id.block, token_id.tx),
            0,
            None,
            None
        ).await?;
        
        debug!("Trace result: {}", serde_json::to_string_pretty(&trace_result)?);
        
        // Parse the trace result to extract token information
        // This is a simplified implementation - in practice, you'd need to decode the actual contract state
        Ok(TokenInfo {
            alkane_id: token_id.clone(),
            name: "Unknown Token".to_string(),
            symbol: "UNK".to_string(),
            total_supply: 0,
            cap: 0,
            amount_per_mint: 0,
            minted: 0,
        })
    }

    /// Get token balance for an address
    pub async fn get_token_balance(&self, token_id: &AlkaneId, address: &str) -> Result<u64> {
        info!("Getting balance for token {}:{} at address {}", 
              token_id.block, token_id.tx, address);
        
        let result = self.rpc_client.get_protorunes_by_address(address).await?;
        
        if let Some(runes_array) = result.as_array() {
            for rune in runes_array {
                if let Some(rune_obj) = rune.as_object() {
                    // Check if this is the token we're looking for
                    if let Some(id_str) = rune_obj.get("id").and_then(|v| v.as_str()) {
                        if let Ok(alkane_id) = crate::utils::parse_alkane_id(id_str) {
                            let alkane_id = super::types::AlkaneId {
                                block: alkane_id.block,
                                tx: alkane_id.tx
                            };
                            if alkane_id.block == token_id.block && alkane_id.tx == token_id.tx {
                                return Ok(rune_obj.get("balance")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .unwrap_or(0));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(0) // Token not found or no balance
    }
}

/// Parse token amounts from string format "block:tx:amount,block:tx:amount,..."
pub fn parse_token_amounts(tokens_str: &str) -> Result<Vec<TokenAmount>> {
    let mut token_amounts = Vec::new();

    if tokens_str.is_empty() {
        return Ok(token_amounts);
    }
    
    if tokens_str.trim().is_empty() {
        return Ok(token_amounts);
    }
    
    for token_part in tokens_str.split(',') {
        let trimmed = token_part.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() != 3 {
            return Err(crate::DeezelError::Parse("Invalid token amount format. Expected 'block:tx:amount'".to_string()));
        }
        
        let block = parts[0].parse::<u64>()
            .context("Invalid block number in token amount")?;
        let tx = parts[1].parse::<u64>()
            .context("Invalid transaction number in token amount")?;
        let amount = parts[2].parse::<u64>()
            .context("Invalid amount in token amount")?;
        
        token_amounts.push(TokenAmount {
            alkane_id: AlkaneId { block, tx },
            amount,
        });
    }
    
    Ok(token_amounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_token_amounts() {
        let amounts = parse_token_amounts("123:456:1000,789:012:2000").unwrap();
        assert_eq!(amounts.len(), 2);
        
        assert_eq!(amounts[0].alkane_id.block, 123);
        assert_eq!(amounts[0].alkane_id.tx, 456);
        assert_eq!(amounts[0].amount, 1000);
        
        assert_eq!(amounts[1].alkane_id.block, 789);
        assert_eq!(amounts[1].alkane_id.tx, 12);
        assert_eq!(amounts[1].amount, 2000);
    }

    #[test]
    fn test_parse_invalid_token_amounts() {
        assert!(parse_token_amounts("invalid").is_err());
        assert!(parse_token_amounts("123:456").is_err());
        assert!(parse_token_amounts("123:456:1000:extra").is_err());
    }

    #[test]
    fn test_parse_empty_token_amounts() {
        let amounts = parse_token_amounts("").unwrap();
        assert_eq!(amounts.len(), 0);
    }
}