use anyhow::{Context, Result};
use redis::{aio::MultiplexedConnection, AsyncCommands, Client};
use std::env;
use tracing::{debug, warn};

/// Build the redis key name using NETWORK (fall back to mainnet to match spec)
fn pools_lastblock_key() -> String {
    let network_env = env::var("NETWORK_ENV")
        .ok()
        .unwrap_or_else(|| "mainnet".to_string());
    format!("indexer-{}-pools-lastblock", network_env)
}

/// Build the redis pub-sub channel name for processed blocks (scoped by network)
fn processed_blocks_channel() -> String {
    let network_env = env::var("NETWORK_ENV")
        .ok()
        .unwrap_or_else(|| "mainnet".to_string());
    format!("indexer-{}-processed-blocks", network_env)
}

async fn get_connection() -> Result<MultiplexedConnection> {
    let url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let client = Client::open(url).context("failed to create redis client")?;
    let conn = client
        .get_multiplexed_tokio_connection()
        .await
        .context("failed to connect to redis")?;
    Ok(conn)
}

/// Best-effort: set the pools last processed block height in Redis.
/// Non-fatal: logs warning on failure.
pub async fn notify_pools_processed(height: u64) {
    let key = pools_lastblock_key();
    match get_connection().await {
        Ok(mut conn) => {
            let value = height.to_string();
            match conn.set::<_, _, ()>(&key, &value).await {
                Ok(()) => debug!(key = %key, value = %value, "redis notify set OK"),
                Err(e) => warn!(key = %key, error = %e, "redis notify set failed"),
            }
        }
        Err(e) => {
            warn!(error = %e, "redis notify connection failed");
        }
    }
}

/// Best-effort: publish a message to the processed-blocks channel with the height.
/// Non-fatal: logs warning on failure.
pub async fn publish_block_processed(height: u64) {
    let channel = processed_blocks_channel();
    match get_connection().await {
        Ok(mut conn) => {
            let message = height.to_string();
            match conn.publish::<_, _, i64>(&channel, &message).await {
                Ok(_receivers) => debug!(channel = %channel, message = %message, "redis pubsub publish OK"),
                Err(e) => warn!(channel = %channel, error = %e, "redis pubsub publish failed"),
            }
        }
        Err(e) => {
            warn!(error = %e, "redis pubsub connection failed");
        }
    }
}


