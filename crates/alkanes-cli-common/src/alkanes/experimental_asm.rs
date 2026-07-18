//! Experimental AssemblyScript WASM-based operations
//! 
//! This module provides high-performance parallel implementations using
//! pre-compiled AssemblyScript WASM for common alkanes operations.

use anyhow::Result;
use futures::stream::{self, StreamExt};
use crate::alkanes::pool_details::{PoolInfo, PoolDetails};
use crate::traits::{AlkanesProvider, DeezelProvider};
use serde::{Serialize, Deserialize};

/// Embedded get-all-pools WASM (compiled from AssemblyScript)
const GET_ALL_POOLS_WASM: &[u8] = include_bytes!("asc/get-all-pools/build/release.wasm");

/// Embedded get-all-pools-details WASM (compiled from AssemblyScript)
const GET_ALL_POOLS_DETAILS_WASM: &[u8] = include_bytes!("asc/get-all-pools-details/build/release.wasm");

/// Alkane metadata reflection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneReflection {
    /// Alkane ID (block:tx)
    pub id: String,
    /// Name from opcode 99 (optional)
    pub name: Option<String>,
    /// Symbol from opcode 100 (optional)
    pub symbol: Option<String>,
    /// Total supply from opcode 101 (optional)
    pub total_supply: Option<u128>,
    /// Cap from opcode 102 (optional)
    pub cap: Option<u128>,
    /// Minted from opcode 103 (optional)
    pub minted: Option<u128>,
    /// Value per mint from opcode 104 (optional)
    pub value_per_mint: Option<u128>,
    /// Additional data from opcode 1000 (optional, as hex)
    pub data: Option<String>,
    /// Premine amount (derived from initial total_supply or contract-specific, optional)
    pub premine: Option<u128>,
    /// Decimals (always 8 for alkanes, included for consistency)
    pub decimals: u8,
}

/// Configuration for parallel pool fetching
#[derive(Debug, Clone)]
pub struct ParallelFetchConfig {
    /// Number of pools to fetch per chunk
    pub chunk_size: usize,
    /// Maximum number of concurrent requests
    pub max_concurrent: usize,
    /// Optional range to fetch (start_index, end_index)
    pub range: Option<(usize, usize)>,
}

impl Default for ParallelFetchConfig {
    fn default() -> Self {
        Self {
            chunk_size: 30,
            max_concurrent: 3,
            range: None,
        }
    }
}

/// Get all pools from a factory using AssemblyScript WASM
/// 
/// # Arguments
/// * `provider` - The alkanes provider to use for RPC calls
/// 
/// # Returns
/// A vector of pool IDs (block, tx) and the total count
pub async fn get_all_pools(provider: &dyn AlkanesProvider) -> Result<(usize, Vec<(u64, u64)>)> {
    // Call get-all-pools WASM
    let pool_list_data = provider.tx_script(GET_ALL_POOLS_WASM, vec![], None).await?;
    
    // Parse: [pool_count(16)][pool0_block(16)][pool0_tx(16)]...
    if pool_list_data.len() < 16 {
        return Err(anyhow::anyhow!("Invalid pool list response"));
    }
    
    let total_pools = u128::from_le_bytes(pool_list_data[0..16].try_into()?) as usize;
    
    // Parse pool IDs
    let mut pools = Vec::new();
    let mut offset = 16;
    
    while offset + 32 <= pool_list_data.len() {
        let block = u128::from_le_bytes(pool_list_data[offset..offset+16].try_into()?) as u64;
        let tx = u128::from_le_bytes(pool_list_data[offset+16..offset+32].try_into()?) as u64;
        pools.push((block, tx));
        offset += 32;
    }
    
    Ok((total_pools, pools))
}

/// Get all pools with details using parallel AssemblyScript WASM execution
/// 
/// This function fetches pool details in parallel chunks for optimal performance.
/// It automatically batches requests and respects rate limits through configurable concurrency.
/// 
/// # Arguments
/// * `provider` - The alkanes provider to use for RPC calls
/// * `config` - Configuration for parallel fetching (chunk size, concurrency, range)
/// 
/// # Returns
/// A vector of PoolInfo structs containing pool IDs and their details
/// 
/// # Example
/// ```no_run
/// use alkanes_cli_common::alkanes::experimental_asm::{get_all_pools_with_details_parallel, ParallelFetchConfig};
/// 
/// # async fn example(provider: &(dyn alkanes_cli_common::traits::DeezelProvider + Send + Sync)) -> anyhow::Result<()> {
/// let config = ParallelFetchConfig {
///     chunk_size: 30,
///     max_concurrent: 3,
///     range: Some((0, 49)), // Fetch first 50 pools
/// };
/// 
/// let pools = get_all_pools_with_details_parallel(provider, config).await?;
/// println!("Fetched {} pools", pools.len());
/// # Ok(())
/// # }
/// ```
pub async fn get_all_pools_with_details_parallel(
    provider: &(dyn DeezelProvider + Send + Sync),
    config: ParallelFetchConfig,
) -> Result<Vec<PoolInfo>> {
    // Step 1: Get total pool count
    let (total_pools, _) = get_all_pools(provider).await?;
    
    // Step 2: Determine range to fetch
    let (start, end) = match config.range {
        Some((s, e)) => (s, e.min(total_pools.saturating_sub(1))),
        None => (0, total_pools.saturating_sub(1)),
    };
    
    if start > end {
        return Ok(Vec::new());
    }
    
    let pools_to_fetch = end - start + 1;
    
    // Step 3: Create chunks
    let mut chunks = Vec::new();
    for chunk_start in (start..=end).step_by(config.chunk_size) {
        let chunk_end = (chunk_start + config.chunk_size - 1).min(end);
        chunks.push((chunk_start, chunk_end));
    }
    
    // Step 4: Fetch chunks in parallel with concurrency limit
    let provider_box = provider.clone_box();
    let results = stream::iter(chunks.into_iter())
        .map(|(chunk_start, chunk_end)| {
            let provider = provider_box.clone();
            let wasm = GET_ALL_POOLS_DETAILS_WASM.to_vec();
            async move {
                let result = provider.tx_script(
                    &wasm,
                    vec![chunk_start as u128, chunk_end as u128],
                    None,
                ).await;
                (chunk_start, chunk_end, result)
            }
        })
        .buffer_unordered(config.max_concurrent)
        .collect::<Vec<_>>()
        .await;
    
    // Step 5: Parse results
    let mut all_pools = Vec::new();
    
    for (chunk_start, chunk_end, result) in results {
        let response_data = match result {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Chunk {}-{} failed: {}", chunk_start, chunk_end, e);
                continue;
            }
        };
        
        // Parse: [count(16)][pool0_id(32)][size0(8)][data0][pool1_id(32)][size1(8)][data1]...
        if response_data.len() < 16 {
            log::warn!("Chunk {}-{}: Invalid response size", chunk_start, chunk_end);
            continue;
        }
        
        let pool_count_in_chunk = u128::from_le_bytes(response_data[0..16].try_into()?) as usize;
        let mut offset = 16;
        
        for _ in 0..pool_count_in_chunk {
            // Read pool ID (32 bytes: 16 for block, 16 for tx)
            if offset + 32 > response_data.len() {
                break;
            }
            
            let pool_block = u128::from_le_bytes(response_data[offset..offset+16].try_into()?) as u64;
            let pool_tx = u128::from_le_bytes(response_data[offset+16..offset+32].try_into()?) as u64;
            offset += 32;
            
            // Read size of this pool's details
            if offset + 8 > response_data.len() {
                break;
            }
            let details_size = u64::from_le_bytes(response_data[offset..offset+8].try_into()?) as usize;
            offset += 8;
            
            if offset + details_size > response_data.len() {
                break;
            }
            
            // Parse pool details using existing PoolDetails::from_bytes
            let details_bytes = &response_data[offset..offset+details_size];
            if let Ok(details) = PoolDetails::from_bytes(details_bytes) {
                all_pools.push(PoolInfo {
                    pool_id_block: pool_block,
                    pool_id_tx: pool_tx,
                    details: Some(details),
                });
            }
            
            offset += details_size;
        }
    }
    
    Ok(all_pools)
}

/// Get all pools with details (sequential, single request)
/// 
/// This is a convenience wrapper around get_all_pools_with_details_parallel
/// with a very high chunk_size to fetch everything in one request.
/// Note: This may timeout or hit fuel limits for large numbers of pools.
/// Use the parallel version for better reliability.
pub async fn get_all_pools_with_details(
    provider: &(dyn DeezelProvider + Send + Sync),
    range: Option<(usize, usize)>,
) -> Result<Vec<PoolInfo>> {
    get_all_pools_with_details_parallel(
        provider,
        ParallelFetchConfig {
            chunk_size: 1000, // Large chunk to fetch in one go
            max_concurrent: 1,
            range,
        },
    ).await
}

/// Standard view opcodes for alkanes metadata
const OPCODE_GET_NAME: u64 = 99;
const OPCODE_GET_SYMBOL: u64 = 100;
const OPCODE_GET_TOTAL_SUPPLY: u64 = 101;
const OPCODE_GET_CAP: u64 = 102;
const OPCODE_GET_MINTED: u64 = 103;
const OPCODE_GET_VALUE_PER_MINT: u64 = 104;
const OPCODE_GET_DATA: u64 = 1000;

/// Reflect metadata for a single alkane by calling standard view opcodes
/// 
/// This function makes parallel RPC calls to query all standard view opcodes:
/// - Opcode 99: GetName
/// - Opcode 100: GetSymbol
/// - Opcode 101: GetTotalSupply
/// - Opcode 102: GetCap
/// - Opcode 103: GetMinted
/// - Opcode 104: GetValuePerMint
/// - Opcode 1000: GetData
/// 
/// Opcodes that are not implemented will have `None` values in the result.
///
/// # Arguments
/// * `provider` - The alkanes provider to use for RPC calls
/// * `alkane_id` - The alkane ID in "block:tx" format
/// * `concurrency` - Maximum number of concurrent RPC calls
///
/// # Returns
/// An AlkaneReflection struct with all available metadata
pub async fn reflect_alkane(
    provider: &dyn DeezelProvider,
    alkane_id: &str,
    concurrency: usize,
) -> Result<AlkaneReflection> {
    use crate::proto::alkanes::MessageContextParcel;
    use crate::traits::AlkanesProvider;
    use prost::Message;

    // Parse alkane ID
    let parts: Vec<&str> = alkane_id.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid alkane_id format. Expected 'block:tx'"));
    }
    let block: u64 = parts[0].parse()?;
    let tx: u64 = parts[1].parse()?;

    // Get current height
    let simulation_height = provider.get_metashrew_height().await?;

    // Build list of opcodes to query
    let opcodes = vec![
        OPCODE_GET_NAME,
        OPCODE_GET_SYMBOL,
        OPCODE_GET_TOTAL_SUPPLY,
        OPCODE_GET_CAP,
        OPCODE_GET_MINTED,
        OPCODE_GET_VALUE_PER_MINT,
        OPCODE_GET_DATA,
    ];

    // Create tasks for parallel execution
    let tasks = opcodes.into_iter().map(|opcode| {
        let provider_clone = provider.clone_box();
        let alkane_id_str = alkane_id.to_string();
        async move {
            // Build calldata for this opcode
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, block).unwrap();
            leb128::write::unsigned(&mut calldata, tx).unwrap();
            leb128::write::unsigned(&mut calldata, opcode).unwrap();

            // Create context
            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: simulation_height,
                vout: 0,
                txindex: 1,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };

            // Make the RPC call
            let result = provider_clone.simulate(&alkane_id_str, &context, Some("latest".to_string())).await;
            (opcode, result)
        }
    });

    // Execute all queries in parallel with concurrency limit
    let results = stream::iter(tasks)
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    // Parse results
    let mut reflection = AlkaneReflection {
        id: alkane_id.to_string(),
        name: None,
        symbol: None,
        total_supply: None,
        cap: None,
        minted: None,
        value_per_mint: None,
        data: None,
        premine: None,
        decimals: 8, // Always 8 for alkanes
    };

    for (opcode, result) in results {
        if let Ok(json) = result {
            if let Some(hex_str) = json.as_str() {
                let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
                if let Ok(bytes) = hex::decode(hex_data) {
                    if let Ok(sim_response) = crate::proto::alkanes::SimulateResponse::decode(bytes.as_slice()) {
                        if let Some(execution) = sim_response.execution {
                            let data = execution.data;
                            
                            match opcode {
                                OPCODE_GET_NAME => {
                                    reflection.name = String::from_utf8(data).ok();
                                }
                                OPCODE_GET_SYMBOL => {
                                    reflection.symbol = String::from_utf8(data).ok();
                                }
                                OPCODE_GET_TOTAL_SUPPLY => {
                                    if data.len() >= 16 {
                                        reflection.total_supply = Some(u128::from_le_bytes(data[0..16].try_into().unwrap()));
                                    }
                                }
                                OPCODE_GET_CAP => {
                                    if data.len() >= 16 {
                                        reflection.cap = Some(u128::from_le_bytes(data[0..16].try_into().unwrap()));
                                    }
                                }
                                OPCODE_GET_MINTED => {
                                    if data.len() >= 16 {
                                        reflection.minted = Some(u128::from_le_bytes(data[0..16].try_into().unwrap()));
                                    }
                                }
                                OPCODE_GET_VALUE_PER_MINT => {
                                    if data.len() >= 16 {
                                        reflection.value_per_mint = Some(u128::from_le_bytes(data[0..16].try_into().unwrap()));
                                    }
                                }
                                OPCODE_GET_DATA => {
                                    reflection.data = Some(hex::encode(&data));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // Derive premine from total_supply if available
    // For genesis alkanes, premine typically equals initial total_supply
    // For fair-launch tokens, premine is 0
    if let Some(total_supply) = reflection.total_supply {
        if let Some(minted) = reflection.minted {
            // If minted is 0 and total_supply > 0, it's likely a premine
            if minted == 0 && total_supply > 0 {
                reflection.premine = Some(total_supply);
            } else if total_supply > minted {
                // Premine is the difference between total_supply and minted
                reflection.premine = Some(total_supply - minted);
            }
        } else if total_supply > 0 {
            // If we don't have minted data but have total_supply, assume it's premine
            reflection.premine = Some(total_supply);
        }
    }

    Ok(reflection)
}

/// Reflect metadata for a range of alkanes
///
/// # Arguments
/// * `provider` - The alkanes provider to use for RPC calls
/// * `block` - The block number
/// * `start_tx` - Starting transaction index (inclusive)
/// * `end_tx` - Ending transaction index (inclusive)
/// * `concurrency` - Maximum number of concurrent RPC calls
///
/// # Returns
/// A vector of AlkaneReflection structs
pub async fn reflect_alkane_range(
    provider: &dyn DeezelProvider,
    block: u64,
    start_tx: u64,
    end_tx: u64,
    concurrency: usize,
) -> Result<Vec<AlkaneReflection>> {
    // Create tasks for each alkane in the range
    let concurrency_per_alkane = 7; // Per-alkane opcode concurrency (we parallelize across alkanes)
    let tasks = (start_tx..=end_tx).map(move |tx| {
        let alkane_id = format!("{}:{}", block, tx);
        async move {
            reflect_alkane(provider, &alkane_id, concurrency_per_alkane).await
        }
    });

    // Execute all reflections in parallel with concurrency limit
    let results = stream::iter(tasks)
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    // Collect successful results (skip failures)
    let mut reflections = Vec::new();
    for result in results {
        match result {
            Ok(reflection) => reflections.push(reflection),
            Err(e) => {
                eprintln!("Warning: Failed to reflect alkane: {}", e);
            }
        }
    }

    Ok(reflections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_embedded() {
        assert!(!GET_ALL_POOLS_WASM.is_empty(), "get-all-pools WASM should be embedded");
        assert!(!GET_ALL_POOLS_DETAILS_WASM.is_empty(), "get-all-pools-details WASM should be embedded");
    }

    #[test]
    fn test_default_config() {
        let config = ParallelFetchConfig::default();
        assert_eq!(config.chunk_size, 30);
        assert_eq!(config.max_concurrent, 3);
        assert_eq!(config.range, None);
    }
}
