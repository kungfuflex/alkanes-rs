//! Experimental AssemblyScript WASM-based operations
//! 
//! This module provides high-performance parallel implementations using
//! pre-compiled AssemblyScript WASM for common alkanes operations.

use anyhow::Result;
use futures::stream::{self, StreamExt};
use crate::alkanes::pool_details::{PoolInfo, PoolDetails};
use crate::traits::{AlkanesProvider, DeezelProvider};

/// Embedded get-all-pools WASM (compiled from AssemblyScript)
const GET_ALL_POOLS_WASM: &[u8] = include_bytes!("asc/get-all-pools/build/release.wasm");

/// Embedded get-all-pools-details WASM (compiled from AssemblyScript)
const GET_ALL_POOLS_DETAILS_WASM: &[u8] = include_bytes!("asc/get-all-pools-details/build/release.wasm");

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
