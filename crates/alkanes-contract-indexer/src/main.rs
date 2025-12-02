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
        cfg.sandshrew_rpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    )
    .await?;

    // Pipeline and poller
    // Bootstrap kv storage used for progress tracking
    progress::ensure_kv_table(&pool).await?;

    let pipeline = pipeline::Pipeline::new(
        pool.clone(),
        cfg.factory_block_id.clone(),
        cfg.factory_tx_id.clone(),
    );
    let progress_store = progress::ProgressStore::new(pool.clone());
    let _last_processed = progress_store.get_last_processed_height().await?;

    // Spawn tip poller (always triggers pools fetch; also processes blocks when following tip)
    let tip_provider = provider;
    let poller_pipeline = pipeline.clone();
    // If we have a configured start height, coordinate catch-up to start only after
    // the poller has initialized metashrew height and refreshed pools.
    let (poller_init_tx, maybe_init_rx) = if cfg.start_height.is_some() {
        let (tx, rx) = oneshot::channel::<()>();
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };
    let poller_fut = async move {
        let poller = poller::BlockPoller::new(
            tip_provider,
            poller_pipeline,
            cfg.poll_interval_ms,
            poller_init_tx,
            cfg.start_height,
        );
        poller.run().await;
    };

    // Spawn catch-up coordinator (sequential processing until tip)
    let coord_provider = provider::build_provider(
        cfg.bitcoin_rpc_url.clone(),
        cfg.sandshrew_rpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    )
    .await?;
    let coordinator = coordinator::CatchUpCoordinator::new(coord_provider, pipeline, progress_store, cfg.start_height);
    let coordinator_fut = async move {
        if let Some(rx) = maybe_init_rx {
            let _ = rx.await; // wait for initial pools refresh + height init
        }
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
