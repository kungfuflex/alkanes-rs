use anyhow::Result;
use alkanes_cli_sys::SystemAlkanes as ConcreteProvider;
use alkanes_cli_common::alkanes::experimental_asm::{get_all_pools_with_details_parallel, ParallelFetchConfig};
use alkanes_cli_common::traits::DeezelProvider;
use tracing::{debug, info, warn};
use sqlx::PgPool;
use crate::db::{pools as db_pools, pool_state as db_pool_state};

#[derive(Debug, Clone)]
pub struct TypesAlkaneId { pub block: String, pub tx: String }

#[derive(Debug, Clone)]
pub struct PoolDetailsResult {
    pub token0: TypesAlkaneId,
    pub token1: TypesAlkaneId,
    pub token0_amount: u128,
    pub token1_amount: u128,
    pub token_supply: u128,
    pub pool_name: String,
}

#[derive(Debug, Clone)]
pub struct PoolWithDetails {
    pub pool_block: String,
    pub pool_tx: String,
    pub details: PoolDetailsResult,
}

pub async fn fetch_all_pools_with_details(
    provider: &ConcreteProvider,
    _factory_block: &str,
    _factory_tx: &str,
) -> Result<Vec<PoolWithDetails>> {
    debug!("fetch pools via experimental ASM with parallel mode (chunk_size=30, concurrency=10)");

    // Use experimental ASM parallel implementation
    let config = ParallelFetchConfig {
        chunk_size: 30,
        max_concurrent: 10,
        range: None, // Fetch all pools
    };

    let pools = get_all_pools_with_details_parallel(provider, config).await?;

    info!(pool_count = pools.len(), "fetched all pools with details via experimental ASM");

    // Convert to our internal format
    let mut out: Vec<PoolWithDetails> = Vec::new();
    for pool_info in pools {
        if let Some(details) = pool_info.details {
            out.push(PoolWithDetails {
                pool_block: pool_info.pool_id_block.to_string(),
                pool_tx: pool_info.pool_id_tx.to_string(),
                details: PoolDetailsResult {
                    token0: TypesAlkaneId { 
                        block: details.token_a_block.to_string(), 
                        tx: details.token_a_tx.to_string() 
                    },
                    token1: TypesAlkaneId { 
                        block: details.token_b_block.to_string(), 
                        tx: details.token_b_tx.to_string() 
                    },
                    token0_amount: details.reserve_a,
                    token1_amount: details.reserve_b,
                    token_supply: details.total_supply,
                    pool_name: details.pool_name,
                },
            });
        } else {
            warn!(pool_block = pool_info.pool_id_block, pool_tx = pool_info.pool_id_tx, "pool has no details, skipping");
        }
    }

    Ok(out)
}


// End-to-end helper: fetch pools via provider and upsert DB state snapshots for a given tip
pub async fn fetch_and_upsert_pools_for_tip(
    provider: &ConcreteProvider,
    pool: &PgPool,
    factory_block_id: &str,
    factory_tx_id: &str,
    tip_height: u64,
) -> Result<()> {
    let fetched = fetch_all_pools_with_details(provider, factory_block_id, factory_tx_id).await?;
    if fetched.is_empty() {
        return Ok(());
    }

    // DB upserts in a transaction
    let mut txdb = pool.begin().await?;

    // Existing pools map for this factory
    let existing_map = db_pools::get_existing_pools_for_factory(&mut txdb, factory_block_id, factory_tx_id).await?;

    // Identify new pools
    let new_pools: Vec<(String, String, String, String, String, String, String)> = fetched.iter()
        .filter(|it| !existing_map.contains_key(&(it.pool_block.clone(), it.pool_tx.clone())))
        .map(|it| (
            it.pool_block.clone(),
            it.pool_tx.clone(),
            it.details.token0.block.clone(),
            it.details.token0.tx.clone(),
            it.details.token1.block.clone(),
            it.details.token1.tx.clone(),
            it.details.pool_name.clone(),
        ))
        .collect();

    if !new_pools.is_empty() {
        db_pools::insert_new_pools(&mut txdb, factory_block_id, factory_tx_id, &new_pools).await?;
    }

    // Fetch all pool DB IDs for the set of pools we have details for
    let pool_pairs: Vec<(String, String)> = fetched.iter().map(|p| (p.pool_block.clone(), p.pool_tx.clone())).collect();
    let id_map = db_pools::get_pool_ids_for_pairs(&mut txdb, factory_block_id, factory_tx_id, &pool_pairs).await?;

    // Fetch latest PoolState per pool
    let id_vec: Vec<&str> = id_map.values().map(|s| s.as_str()).collect();
    let last_state = db_pool_state::get_latest_pool_states(&mut txdb, &id_vec).await?;

    // Prepare new snapshots for changed states
    let mut snapshots: Vec<(String, i32, String, String, String)> = Vec::new();
    for item in &fetched {
        if let Some(pool_id) = id_map.get(&(item.pool_block.clone(), item.pool_tx.clone())) {
            let new_t0 = item.details.token0_amount.to_string();
            let new_t1 = item.details.token1_amount.to_string();
            let new_sup = item.details.token_supply.to_string();
            let changed = match last_state.get(pool_id) {
                Some((t0, t1, sup)) => t0 != &new_t0 || t1 != &new_t1 || sup != &new_sup,
                None => true,
            };
            if changed {
                snapshots.push((pool_id.clone(), tip_height as i32, new_t0, new_t1, new_sup));
            }
        }
    }

    // Batch insert snapshots
    if !snapshots.is_empty() {
        db_pool_state::insert_pool_state_snapshots(&mut txdb, &snapshots).await?;
    }

    txdb.commit().await?;
    info!(height = tip_height, pools = fetched.len(), inserts = snapshots.len(), "pools and states updated");
    Ok(())
}

