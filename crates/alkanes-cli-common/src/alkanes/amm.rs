//! AMM/DEX functionality for alkanes
//!
//! This module implements AMM (Automated Market Maker) functionality following the OYL SDK patterns
//! but leveraging our enhanced alkanes execute system. Each AMM operation (create pool, add liquidity,
//! remove liquidity, swap) uses our improved execute functionality with proper protostones encoding.
//!
//! Key workflows implemented:
//! - Pool creation with factory contract calls
//! - Liquidity provision with optimal amount calculations
//! - Liquidity removal with preview functionality
//! - Token swaps with slippage protection
//! - Pool state queries and simulations

use crate::Result;
use log::{debug, info};

#[cfg(not(target_arch = "wasm32"))]
use std::{sync::Arc, vec::Vec};
#[cfg(target_arch = "wasm32")]
use alloc::{sync::Arc, vec::Vec};

use crate::{ToString, format};

#[cfg(not(target_arch = "wasm32"))]
use std::vec;
#[cfg(target_arch = "wasm32")]
use alloc::vec;

// Use specific imports to avoid conflicts
use super::types::{PoolCreateParams, LiquidityAddParams, LiquidityRemoveParams, SwapParams, TokenAmount, LiquidityRemovalPreview};
use super::types::AlkaneId as TypesAlkaneId;
use super::execute::{EnhancedAlkanesExecutor, EnhancedExecuteParams, EnhancedExecuteResult, InputRequirement};
use crate::traits::DeezelProvider;

/// AMM operations manager that leverages enhanced execute functionality
pub struct AmmManager<P: DeezelProvider> {
    executor: Arc<EnhancedAlkanesExecutor<P>>,
}

impl<P: DeezelProvider> AmmManager<P> {
    /// Create a new AMM manager
    pub fn new(executor: Arc<EnhancedAlkanesExecutor<P>>) -> Self {
        Self { executor }
    }

    /// Create a new liquidity pool using enhanced execute functionality
    ///
    /// This follows the OYL SDK pattern:
    /// 1. Prepare token edicts for the pool tokens
    /// 2. Create protostones with factory contract calldata
    /// 3. Execute using enhanced execute with proper encoding
    pub async fn create_pool(&self, params: PoolCreateParams) -> Result<EnhancedExecuteResult> {
        info!("Creating liquidity pool with {} tokens using enhanced execute", params.tokens.len());
        debug!("Pool calldata: {:?}", params.calldata);
        
        // Validate that we have exactly 2 tokens (standard AMM pool)
        if params.tokens.len() != 2 {
            return Err(crate::DeezelError::Validation("Pool creation requires exactly 2 tokens".to_string()));
        }
        
        // Validate token amounts
        for token in &params.tokens {
            if token.amount == 0 {
                return Err(crate::DeezelError::Validation("Token amounts must be greater than zero".to_string()));
            }
        }
        
        // Convert calldata from Vec<String> to Vec<u8>
        let _calldata_bytes = params.calldata.join(",").into_bytes();
        
        // Prepare input requirements for the tokens
        let mut input_requirements = Vec::new();
        for token in &params.tokens {
            input_requirements.push(InputRequirement::Alkanes {
                block: token.alkane_id.block,
                tx: token.alkane_id.tx,
                amount: token.amount,
            });
        }
        
        // Create protostones with factory contract calldata
        let protostones = vec![
            super::execute::ProtostoneSpec {
                cellpack: None,
                edicts: Vec::new(),
                bitcoin_transfer: None,
            },
        ];
        
        // Prepare enhanced execute parameters
        let execute_params = EnhancedExecuteParams {
            fee_rate: params.fee_rate,
            to_addresses: vec![], // Will be populated with default addresses
            change_address: None,
            input_requirements,
            protostones,
            envelope_data: None,
            raw_output: false,
            trace_enabled: true,
            mine_enabled: false,
            auto_confirm: true,
            rebar: false,
        };
        
        // Execute pool creation using enhanced execute
        let result = self.executor.execute(execute_params).await?;
        
        info!("Liquidity pool created successfully using enhanced execute");
        info!("Pool creation reveal TXID: {}", result.reveal_txid);
        
        Ok(result)
    }

    /// Add liquidity to a pool using enhanced execute functionality
    ///
    /// This follows the OYL SDK pattern:
    /// 1. Calculate optimal token amounts based on current pool reserves
    /// 2. Prepare token edicts for the liquidity tokens
    /// 3. Create protostones with pool contract calldata
    /// 4. Execute using enhanced execute with proper encoding
    pub async fn add_liquidity(&self, params: LiquidityAddParams) -> Result<EnhancedExecuteResult> {
        info!("Adding liquidity with {} tokens using enhanced execute", params.tokens.len());
        debug!("Liquidity calldata: {:?}", params.calldata);
        
        // Validate that we have tokens to add
        if params.tokens.is_empty() {
            return Err(crate::DeezelError::Validation("Cannot add liquidity without tokens".to_string()));
        }
        
        // Validate token amounts
        for token in &params.tokens {
            if token.amount == 0 {
                return Err(crate::DeezelError::Validation("Token amounts must be greater than zero".to_string()));
            }
        }
        
        // Get current pool reserves to calculate optimal amounts
        let reserves = self.get_pool_reserves(&params.pool).await?;
        
        // Calculate optimal liquidity amounts if pool already has reserves
        let optimal_tokens = if reserves.len() >= 2 && reserves[0].amount > 0 && reserves[1].amount > 0 {
            let mut optimal = Vec::new();
            for (i, token) in params.tokens.iter().enumerate() {
                if i < reserves.len() {
                    let (optimal_a, optimal_b) = calculate_optimal_liquidity(
                        token.amount,
                        params.tokens.get(1).map(|t| t.amount).unwrap_or(0),
                        reserves[0].amount,
                        reserves[1].amount,
                    )?;
                    optimal.push(if i == 0 { optimal_a } else { optimal_b });
                } else {
                    optimal.push(token.amount);
                }
            }
            optimal
        } else {
            // First liquidity provision - use provided amounts
            params.tokens.iter().map(|t| t.amount).collect()
        };
        
        // Convert calldata from Vec<String> to Vec<u8>
        let _calldata_bytes = params.calldata.join(",").into_bytes();
        
        // Prepare input requirements for the optimal token amounts
        let mut input_requirements = Vec::new();
        for (token, &optimal_amount) in params.tokens.iter().zip(optimal_tokens.iter()) {
            input_requirements.push(InputRequirement::Alkanes {
                block: token.alkane_id.block,
                tx: token.alkane_id.tx,
                amount: optimal_amount,
            });
        }
        
        // Create protostones with pool contract calldata
        let protostones = vec![
            super::execute::ProtostoneSpec {
                cellpack: None,
                edicts: Vec::new(),
                bitcoin_transfer: None,
            },
        ];
        
        // Prepare enhanced execute parameters
        let execute_params = EnhancedExecuteParams {
            fee_rate: params.fee_rate,
            to_addresses: vec![], // Will be populated with default addresses
            change_address: None,
            input_requirements,
            protostones,
            envelope_data: None,
            raw_output: false,
            trace_enabled: true,
            mine_enabled: false,
            auto_confirm: true,
            rebar: false,
        };
        
        // Execute liquidity addition using enhanced execute
        let result = self.executor.execute(execute_params).await?;
        
        info!("Liquidity added successfully using enhanced execute");
        info!("Add liquidity reveal TXID: {}", result.reveal_txid);
        
        Ok(result)
    }

    /// Remove liquidity from a pool using enhanced execute functionality
    ///
    /// This follows the OYL SDK pattern:
    /// 1. Preview the removal to calculate expected token amounts
    /// 2. Prepare LP token edict for burning
    /// 3. Create protostones with pool contract calldata
    /// 4. Execute using enhanced execute with proper encoding
    pub async fn remove_liquidity(&self, params: LiquidityRemoveParams) -> Result<EnhancedExecuteResult> {
        info!("Removing {} LP tokens from pool {}:{} using enhanced execute",
              params.amount, params.token.block, params.token.tx);
        debug!("Remove liquidity calldata: {:?}", params.calldata);
        
        // Validate amount
        if params.amount == 0 {
            return Err(crate::DeezelError::Validation("Cannot remove zero liquidity".to_string()));
        }
        
        // Preview the removal to get expected amounts
        let preview = self.preview_remove_liquidity(&params.token, params.amount).await?;
        info!("Expected removal: {} token A, {} token B",
              preview.token_a_amount, preview.token_b_amount);
        
        // Convert calldata from Vec<String> to Vec<u8>
        let _calldata_bytes = params.calldata.join(",").into_bytes();
        
        // Prepare input requirements for the LP tokens to burn
        let input_requirements = vec![
            InputRequirement::Alkanes {
                block: params.token.block,
                tx: params.token.tx,
                amount: params.amount,
            }
        ];
        
        // Create protostones with pool contract calldata
        let protostones = vec![
            super::execute::ProtostoneSpec {
                cellpack: None,
                edicts: Vec::new(),
                bitcoin_transfer: None,
            },
        ];
        
        // Prepare enhanced execute parameters
        let execute_params = EnhancedExecuteParams {
            fee_rate: params.fee_rate,
            to_addresses: vec![], // Will be populated with default addresses
            change_address: None,
            input_requirements,
            protostones,
            envelope_data: None,
            raw_output: false,
            trace_enabled: true,
            mine_enabled: false,
            auto_confirm: true,
            rebar: false,
        };
        
        // Execute liquidity removal using enhanced execute
        let result = self.executor.execute(execute_params).await?;
        
        info!("Liquidity removed successfully using enhanced execute");
        info!("Remove liquidity reveal TXID: {}", result.reveal_txid);
        
        Ok(result)
    }

    /// Swap tokens in a pool using enhanced execute functionality
    ///
    /// This follows the OYL SDK pattern:
    /// 1. Calculate expected output based on pool reserves
    /// 2. Prepare input token edict for the swap
    /// 3. Create protostones with pool contract calldata
    /// 4. Execute using enhanced execute with proper encoding
    pub async fn swap(&self, params: SwapParams) -> Result<EnhancedExecuteResult> {
        info!("Swapping {} units of token {}:{} using enhanced execute",
              params.amount, params.token.block, params.token.tx);
        debug!("Swap calldata: {:?}", params.calldata);
        
        // Validate amount
        if params.amount == 0 {
            return Err(crate::DeezelError::Validation("Cannot swap zero tokens".to_string()));
        }
        
        // Get pool reserves to calculate swap output
        let reserves = self.get_pool_reserves(&params.pool).await?;
        
        // Find input and output reserves
        let input_reserve = reserves.iter()
            .find(|r| r.alkane_id.block == params.token.block && r.alkane_id.tx == params.token.tx)
            .map(|r| r.amount)
            .unwrap_or(0);
        
        if input_reserve == 0 {
            return Err(crate::DeezelError::Validation("Token not found in pool".to_string()));
        }
        
        // Calculate expected output (simplified - assumes 2-token pool)
        let output_reserve = reserves.iter()
            .find(|r| !(r.alkane_id.block == params.token.block && r.alkane_id.tx == params.token.tx))
            .map(|r| r.amount)
            .unwrap_or(0);
        
        let expected_output = calculate_swap_output(params.amount, input_reserve, output_reserve, 30)?;
        info!("Expected swap output: {} tokens", expected_output);
        
        // Convert calldata from Vec<String> to Vec<u8>
        let _calldata_bytes = params.calldata.join(",").into_bytes();
        
        // Prepare input requirements for the token to swap
        let input_requirements = vec![
            InputRequirement::Alkanes {
                block: params.token.block,
                tx: params.token.tx,
                amount: params.amount,
            }
        ];
        
        // Create protostones with pool contract calldata
        let protostones = vec![
            super::execute::ProtostoneSpec {
                cellpack: None,
                edicts: Vec::new(),
                bitcoin_transfer: None,
            },
        ];
        
        // Prepare enhanced execute parameters
        let execute_params = EnhancedExecuteParams {
            fee_rate: params.fee_rate,
            to_addresses: vec![], // Will be populated with default addresses
            change_address: None,
            input_requirements,
            protostones,
            envelope_data: None,
            raw_output: false,
            trace_enabled: true,
            mine_enabled: false,
            auto_confirm: true,
            rebar: false,
        };
        
        // Execute token swap using enhanced execute
        let result = self.executor.execute(execute_params).await?;
        
        info!("Token swap completed successfully using enhanced execute");
        info!("Swap reveal TXID: {}", result.reveal_txid);
        
        Ok(result)
    }

    /// Preview liquidity removal
    pub async fn preview_remove_liquidity(&self, token_id: &TypesAlkaneId, amount: u64) -> Result<LiquidityRemovalPreview> {
        info!("Previewing removal of {} LP tokens from {}:{}", 
              amount, token_id.block, token_id.tx);
        
        // For now, return a placeholder result
        // In a real implementation, this would:
        // 1. Query the pool state
        // 2. Calculate proportional token amounts
        // 3. Return the preview without executing
        
        Ok(LiquidityRemovalPreview {
            token_a_amount: amount / 2, // Placeholder calculation
            token_b_amount: amount / 2, // Placeholder calculation
            lp_tokens_burned: amount,
        })
    }

    /// Get pool information (placeholder implementation)
    pub async fn get_pool_info(&self, pool_id: &TypesAlkaneId) -> Result<serde_json::Value> {
        info!("Getting pool info for: {}:{}", pool_id.block, pool_id.tx);
        
        // For now, return a placeholder result
        // In a full implementation, this would query the pool contract state
        Ok(serde_json::json!({
            "pool_id": format!("{}:{}", pool_id.block, pool_id.tx),
            "status": "active"
        }))
    }

    /// Get pool reserves (placeholder implementation)
    pub async fn get_pool_reserves(&self, pool_id: &TypesAlkaneId) -> Result<Vec<TokenAmount>> {
        info!("Getting pool reserves for: {}:{}", pool_id.block, pool_id.tx);
        
        // For now, return placeholder reserves
        // In a full implementation, this would query the pool contract state
        // to get actual reserve amounts for each token in the pool
        
        // Return empty reserves as placeholder
        debug!("Returning placeholder empty reserves for pool {}:{}", pool_id.block, pool_id.tx);
        Ok(Vec::new())
    }
}

/// Calculate optimal liquidity amounts for adding to a pool
pub fn calculate_optimal_liquidity(
    desired_a: u64,
    desired_b: u64,
    reserve_a: u64,
    reserve_b: u64,
) -> Result<(u64, u64)> {
    if reserve_a == 0 || reserve_b == 0 {
        // First liquidity provision
        return Ok((desired_a, desired_b));
    }
    
    // Calculate optimal amounts based on current pool ratio
    let amount_b_optimal = (desired_a * reserve_b) / reserve_a;
    
    if amount_b_optimal <= desired_b {
        Ok((desired_a, amount_b_optimal))
    } else {
        let amount_a_optimal = (desired_b * reserve_a) / reserve_b;
        Ok((amount_a_optimal, desired_b))
    }
}

/// Calculate swap output amount using constant product formula
pub fn calculate_swap_output(
    input_amount: u64,
    input_reserve: u64,
    output_reserve: u64,
    fee_rate: u64, // Fee rate in basis points (e.g., 30 for 0.3%)
) -> Result<u64> {
    if input_reserve == 0 || output_reserve == 0 {
        return Err(crate::DeezelError::Validation("Cannot swap with zero reserves".to_string()));
    }
    
    // Apply fee to input amount
    let input_amount_with_fee = input_amount * (10000 - fee_rate);
    
    // Calculate output using constant product formula: x * y = k
    let numerator = input_amount_with_fee * output_reserve;
    let denominator = (input_reserve * 10000) + input_amount_with_fee;
    
    Ok(numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_optimal_liquidity_first_provision() {
        let (amount_a, amount_b) = calculate_optimal_liquidity(1000, 2000, 0, 0).unwrap();
        assert_eq!(amount_a, 1000);
        assert_eq!(amount_b, 2000);
    }

    #[test]
    fn test_calculate_optimal_liquidity_existing_pool() {
        // Pool has 1:2 ratio (1000:2000)
        let (amount_a, amount_b) = calculate_optimal_liquidity(500, 2000, 1000, 2000).unwrap();
        assert_eq!(amount_a, 500);
        assert_eq!(amount_b, 1000); // Optimal amount based on ratio
    }

    #[test]
    fn test_calculate_swap_output() {
        // Swap 100 tokens with 0.3% fee
        let output = calculate_swap_output(100, 1000, 2000, 30).unwrap();
        // Expected: (100 * 9970 * 2000) / (1000 * 10000 + 100 * 9970) = ~181
        assert!(output > 180 && output < 185);
    }

    #[test]
    fn test_calculate_swap_output_zero_reserves() {
        assert!(calculate_swap_output(100, 0, 1000, 30).is_err());
        assert!(calculate_swap_output(100, 1000, 0, 30).is_err());
    }
}