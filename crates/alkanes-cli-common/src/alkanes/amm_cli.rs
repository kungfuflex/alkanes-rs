//! CLI-specific AMM operations (init-pool, swap)

use crate::{Result, AlkanesError, DeezelProvider};
use log::info;
use super::types::AlkaneId;
use super::execute::{EnhancedAlkanesExecutor, EnhancedExecuteParams};
use super::parsing::parse_protostones;

/// Parameters for initializing a new pool
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitPoolParams {
    pub factory_id: AlkaneId,
    pub token0: AlkaneId,
    pub token1: AlkaneId,
    pub amount0: u128,
    pub amount1: u128,
    pub minimum_lp: Option<u128>,
    pub to_address: String,
    pub from_address: String,
    pub change_address: Option<String>,
    pub fee_rate: Option<f64>,
    pub trace: bool,
    pub auto_confirm: bool,
    #[serde(default)]
    pub ordinals_strategy: super::types::OrdinalsStrategy,
}

/// Parameters for executing a swap
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwapExecuteParams {
    pub factory_id: AlkaneId,
    pub path: Vec<AlkaneId>, // Token path (minimum 2)
    pub input_amount: u128,
    pub minimum_output: u128,
    pub expires: u64,
    pub to_address: String,
    pub from_address: String,
    pub change_address: Option<String>,
    pub fee_rate: Option<f64>,
    pub trace: bool,
    pub auto_confirm: bool,
    #[serde(default)]
    pub ordinals_strategy: super::types::OrdinalsStrategy,
}

/// Add liquidity to a pool (opcode 1)
/// Calldata format: [factory, 1, token0Block, token0Tx, token1Block, token1Tx, amount0, amount1]:v0:v0
#[cfg(feature = "std")]
pub async fn init_pool(
    provider: &mut dyn DeezelProvider,
    params: InitPoolParams,
) -> Result<String> {
    info!("Initializing pool: {}:{} / {}:{}", 
          params.token0.block, params.token0.tx,
          params.token1.block, params.token1.tx);
    
    // Calculate minimum LP tokens if not provided
    // Formula: sqrt(amount0 * amount1) - MINIMUM_LIQUIDITY (1000)
    let minimum_lp = params.minimum_lp.unwrap_or_else(|| {
        let product = params.amount0 * params.amount1;
        let sqrt = (product as f64).sqrt() as u128;
        sqrt.saturating_sub(1000)
    });
    
    info!("Liquidity: {} token0, {} token1, minimum LP: {}", 
          params.amount0, params.amount1, minimum_lp);
    
    // Build calldata with one protostone that calls factory with opcode 1 (create new pool)
    // The tokens will come from transaction inputs (via UTXOs) automatically
    // If auto-change is needed, it will be inserted as protostone #0 and will send tokens to p1 (this protostone)
    // Format: [factoryBlock,factoryTx,1,token0Block,token0Tx,token1Block,token1Tx,amount0,amount1,...]:v0:v0
    let calldata = format!(
        "[{},{},1,{},{},{},{},{},{}]:v0:v0",
        params.factory_id.block,
        params.factory_id.tx,
        params.token0.block,
        params.token0.tx,
        params.token1.block,
        params.token1.tx,
        params.amount0,
        params.amount1
    );
    
    info!("Calldata: {}", calldata);
    
    // Input requirements: token0 and token1
    // Note: Auth token [2:1] is NOT required for CREATE_NEW_POOL (opcode 1) - anyone can create pools
    // Auth token is only required for InitFactory (opcode 0) which is a one-time initialization
    // These will be handled by alkanes execute's UTXO selection and auto-change logic
    let input_reqs = vec![
        super::types::InputRequirement::Alkanes {
            block: params.token0.block,
            tx: params.token0.tx,
            amount: params.amount0 as u64,
        },
        super::types::InputRequirement::Alkanes {
            block: params.token1.block,
            tx: params.token1.tx,
            amount: params.amount1 as u64,
        },
    ];
    
    let protostones = parse_protostones(&calldata)?;

    // Build execute params with alkanes_change_address set to enable auto-change
    let mut executor = EnhancedAlkanesExecutor::new(provider);
    let execute_params = EnhancedExecuteParams {
        input_requirements: input_reqs,
        alkanes_change_address: Some(params.from_address.clone()), // Enable auto-change for excess alkanes
        to_addresses: vec![params.to_address.clone()],
        from_addresses: Some(vec![params.from_address.clone()]),
        change_address: params.change_address.clone(),
        fee_rate: params.fee_rate.map(|f| f as f32),
        envelope_data: None,
        protostones,
        raw_output: false,
        trace_enabled: params.trace,
        mine_enabled: false,
        auto_confirm: params.auto_confirm,
        ordinals_strategy: params.ordinals_strategy,
        mempool_indexer: false,
        split_transactions: false,
        known_pending_tx_hexes: Vec::new(),
    };
    
    // Execute
    let state = executor.execute(execute_params.clone()).await?;
    let result = match state {
        crate::alkanes::types::ExecutionState::ReadyToSign(ready) => {
            executor.resume_execution(ready, &execute_params).await?
        }
        _ => return Err(AlkanesError::Validation("Unexpected execution state".to_string())),
    };
    
    let txid = result.reveal_txid.clone();
    
    if !params.trace {
        println!("✅ Pool initialized!");
        println!("📝 Transaction: {}", txid);
        println!("🏊 Pool ID will be: {}:{}", params.factory_id.block, "[NEW_TX_ID]");
        println!("💧 Initial liquidity: {} / {}", params.amount0, params.amount1);
        println!("🎫 Minimum LP tokens: {}", minimum_lp);
    }
    
    Ok(txid)
}

/// Execute a token swap (opcode 3)
/// Calldata format: [factory, 3, poolBlock, poolTx]:inputAmount:minimumOutput:expiryBlock
#[cfg(feature = "std")]
pub async fn execute_swap(
    provider: &mut dyn DeezelProvider,
    params: SwapExecuteParams,
) -> Result<String> {
    if params.path.len() < 2 {
        return Err(AlkanesError::Validation("Swap path must have at least 2 tokens".to_string()));
    }
    
    info!("Executing swap: {} → {}", 
          format!("{}:{}", params.path[0].block, params.path[0].tx),
          format!("{}:{}", params.path[params.path.len() - 1].block, params.path[params.path.len() - 1].tx));
    
    // For now, only support direct swaps (single hop)
    if params.path.len() > 2 {
        return Err(AlkanesError::Validation("Multi-hop swaps not yet supported. Use a direct pair.".to_string()));
    }
    
    let input_token = &params.path[0];
    let output_token = &params.path[1];
    
    info!("Swap details: {} {} → min {} {}", 
          params.input_amount, format!("{}:{}", input_token.block, input_token.tx),
          params.minimum_output, format!("{}:{}", output_token.block, output_token.tx));
    
    // Need to find the pool for this pair
    // For now, we'll need the user to provide the pool ID
    // TODO: Add pool lookup from factory
    
    // Build calldata: [factory, 3, poolBlock, poolTx]:inputAmount:minimumOutput:expiryBlock
    // For direct swap, we need to query which pool contains this pair
    // This would require calling the factory contract to find the pool
    
    // Temporary: construct calldata assuming pool discovery
    let calldata = format!(
        "[{},{},3]:{}:{}:{}",
        params.factory_id.block,
        params.factory_id.tx,
        params.input_amount,
        params.minimum_output,
        params.expires
    );
    
    info!("Swap calldata: {}", calldata);
    
    // Parse the input requirements - we need the input token amount
    let input_reqs = vec![
        super::types::InputRequirement::Alkanes {
            block: input_token.block,
            tx: input_token.tx,
            amount: params.input_amount as u64,
        },
    ];
    
    let protostones = parse_protostones(&calldata)?;

    // Build execute params
    let mut executor = EnhancedAlkanesExecutor::new(provider);
    let execute_params = EnhancedExecuteParams {
        input_requirements: input_reqs,
        alkanes_change_address: None,
        to_addresses: vec![params.to_address.clone()],
        from_addresses: Some(vec![params.from_address.clone()]),
        change_address: params.change_address.clone(),
        fee_rate: params.fee_rate.map(|f| f as f32),
        envelope_data: None,
        protostones,
        raw_output: false,
        trace_enabled: params.trace,
        mine_enabled: false,
        auto_confirm: params.auto_confirm,
        ordinals_strategy: params.ordinals_strategy,
        mempool_indexer: false,
        split_transactions: false,
        known_pending_tx_hexes: Vec::new(),
    };
    
    // Execute
    let state = executor.execute(execute_params.clone()).await?;
    let result = match state {
        crate::alkanes::types::ExecutionState::ReadyToSign(ready) => {
            executor.resume_execution(ready, &execute_params).await?
        }
        _ => return Err(AlkanesError::Validation("Unexpected execution state".to_string())),
    };
    
    let txid = result.reveal_txid.clone();
    
    if !params.trace {
        println!("✅ Swap executed!");
        println!("📝 Transaction: {}", txid);
        println!("🔄 Swapping {} of {}:{} → min {} of {}:{}",
                 params.input_amount, input_token.block, input_token.tx,
                 params.minimum_output, output_token.block, output_token.tx);
        println!("⏰ Expires at block: {}", params.expires);
    }
    
    Ok(txid)
}
