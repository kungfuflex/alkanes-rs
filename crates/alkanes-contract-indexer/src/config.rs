use anyhow::Result;
use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub jsonrpc_url: String,
    pub bitcoin_rpc_url: Option<String>,
    pub esplora_url: Option<String>,
    pub network_provider: String,
    pub poll_interval_ms: u64,
    pub start_height: Option<u64>,
    pub factory_block_id: String,
    pub factory_tx_id: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL")?;
        // Support both JSONRPC_URL and legacy SANDSHREW_RPC_URL
        let jsonrpc_url = env::var("JSONRPC_URL")
            .or_else(|_| env::var("SANDSHREW_RPC_URL"))
            .unwrap_or_else(|_| "http://localhost:18888".to_string());
        let bitcoin_rpc_url = env::var("BITCOIN_RPC_URL").ok();
        let esplora_url = env::var("ESPLORA_URL").ok();
        let network_provider = env::var("NETWORK").unwrap_or_else(|_| "regtest".to_string());
        let poll_interval_ms = env::var("POLL_INTERVAL_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2_000);
        let start_height = env::var("START_HEIGHT").ok().and_then(|s| s.parse::<u64>().ok());
        let factory_block_id = env::var("FACTORY_BLOCK_ID").unwrap_or_else(|_| "0".to_string());
        let factory_tx_id = env::var("FACTORY_TX_ID").unwrap_or_else(|_| "0".to_string());

        Ok(Self {
            database_url,
            jsonrpc_url,
            bitcoin_rpc_url,
            esplora_url,
            network_provider,
            poll_interval_ms,
            start_height,
            factory_block_id,
            factory_tx_id,
        })
    }
}


