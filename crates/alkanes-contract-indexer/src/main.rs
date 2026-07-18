use anyhow::Result;
use dotenvy::dotenv;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use tokio::sync::oneshot;

mod config;
mod db;
mod progress;
mod coordinator;
mod pipeline;
mod poller;
mod provider;
mod helpers;
mod inferred_transfers;
mod transform_integration;
use crate::db::blocks::ensure_processed_blocks_table;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Logging
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).init();

    // Config
    let cfg = config::AppConfig::from_env()?;

    // Downstream helpers now accept URL parameters directly; no env mutation needed.

    // DB pool
    let pool = db::connect(&cfg.database_url, 10).await?;
    info!("Connected to Postgres");

    // Ensure ProcessedBlocks exists (defensive)
    ensure_processed_blocks_table(&pool).await?;
    
    // Apply trace transform schema
    info!("Applying trace transform schema...");
    let mut transform_service = transform_integration::TraceTransformService::new(pool.clone());
    transform_service.apply_schema().await?;
    // Load existing pools from database
    transform_service.load_existing_pools().await?;
    info!("Trace transform schema applied and {} pools loaded", transform_service.known_pools.len());

    // Provider
    let provider = provider::build_provider(
        cfg.bitcoin_rpc_url.clone(),
        cfg.jsonrpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    )
    .await?;

    // Pipeline and poller
    // Bootstrap position table used for progress tracking
    progress::ensure_position_table(&pool).await?;
    // Migrate from old kv_store if needed
    progress::migrate_from_kv_store(&pool).await?;

    let pipeline = pipeline::Pipeline::new(
        pool.clone(),
        cfg.factory_block_id.clone(),
        cfg.factory_tx_id.clone(),
    );
    let progress_store = progress::ProgressStore::new(pool.clone());
    let position = progress_store.get_position().await?;
    if let Some(ref pos) = position {
        info!(height = pos.height, block_hash = %pos.block_hash, "resuming from position");
    } else {
        info!("no position found, starting fresh");
    }

    // Spawn tip poller (always triggers pools fetch; also processes blocks when following tip)
    let tip_provider = provider;
    let poller_pipeline = pipeline.clone();
    let poller_progress = progress::ProgressStore::new(pool.clone());
    // Always coordinate catch-up - wait for poller to initialize pools before starting
    let (poller_init_tx, poller_init_rx) = oneshot::channel::<()>();
    let poller_fut = async move {
        let poller = poller::BlockPoller::new(
            tip_provider,
            poller_pipeline,
            poller_progress,
            cfg.poll_interval_ms,
            Some(poller_init_tx),
            cfg.start_height,
        );
        poller.run().await;
    };

    // Spawn catch-up coordinator (sequential processing until tip)
    // This always runs - when no position exists, it starts from start_height or 0
    let coord_provider = provider::build_provider(
        cfg.bitcoin_rpc_url.clone(),
        cfg.jsonrpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    )
    .await?;
    let coordinator = coordinator::CatchUpCoordinator::new(coord_provider, pipeline, progress_store, cfg.start_height);
    let coordinator_fut = async move {
        // Wait for initial pools refresh + height init before starting catch-up
        let _ = poller_init_rx.await;
        loop {
            let _ = coordinator.run_once().await;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    };

    tokio::select! {
        _ = poller_fut => {}
        _ = coordinator_fut => {}
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received");
        }
    }

    Ok(())
}
