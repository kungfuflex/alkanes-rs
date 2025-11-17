use anyhow::Result;
use deezel_common::provider::ConcreteProvider;
use std::sync::Arc;
use deezel_common::alkanes::amm::AmmManager;
use std::env;
use tracing::{debug, info};
use sqlx::PgPool;
use crate::db::{pools as db_pools, pool_state as db_pool_state};
use futures::stream::{self, StreamExt};

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
    factory_block: &str,
    factory_tx: &str,
) -> Result<Vec<PoolWithDetails>> {
    // Use AmmManager helpers which perform raw simulate and decode for us
    let amm = Arc::new(AmmManager::new(Arc::new(provider.clone())));
    // Use SANDSHREW_RPC_URL from environment (.env loaded in main)
    let url = env::var("SANDSHREW_RPC_URL").unwrap_or_else(|_| "http://localhost:18888".to_string());
    debug!(url = %url, factory_block, factory_tx, "fetch pools via AmmManager");

    // Step 1: fetch all pool IDs via raw simulate
    let all = amm
        .get_all_pools_via_raw_simulate(&url, factory_block.to_string(), factory_tx.to_string())
        .await?;

    if all.pools.is_empty() {
        return Ok(Vec::new());
    }

    debug!(count = all.count, "fetched pool ids; fetching details with concurrency = 10");

    // Step 2: concurrently fetch each pool's details with bounded parallelism (10)
    let stream = stream::iter(all.pools.into_iter().map(|id| {
        let amm = amm.clone();
        let url = url.clone();
        async move {
            let res = amm
                .get_pool_details_via_raw_simulate(&url, id.block.to_string(), id.tx.to_string())
                .await;
            (id, res)
        }
    }))
    .buffer_unordered(10);

    let mut out: Vec<PoolWithDetails> = Vec::new();
    tokio::pin!(stream);
    while let Some((pool_id, details_res)) = stream.next().await {
        if let Ok(details) = details_res {
            out.push(PoolWithDetails {
                pool_block: pool_id.block.to_string(),
                pool_tx: pool_id.tx.to_string(),
                details: PoolDetailsResult {
                    token0: TypesAlkaneId { block: details.token0.block.to_string(), tx: details.token0.tx.to_string() },
                    token1: TypesAlkaneId { block: details.token1.block.to_string(), tx: details.token1.tx.to_string() },
                    token0_amount: details.token0_amount as u128,
                    token1_amount: details.token1_amount as u128,
                    token_supply: details.token_supply as u128,
                    pool_name: details.pool_name,
                },
            });
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

