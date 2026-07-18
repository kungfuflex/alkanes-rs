use anyhow::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub sandshrew_url: String,
    pub network_env: String,
    pub infura_endpoint: String,
    pub alkane_factory_id: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("Invalid PORT")?,
            database_url: env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            sandshrew_url: env::var("SANDSHREW_URL").context("SANDSHREW_URL must be set")?,
            network_env: env::var("NETWORK_ENV").unwrap_or_else(|_| "mainnet".to_string()),
            infura_endpoint: env::var("INFURA_ENDPOINT")
                .unwrap_or_else(|_| "https://mainnet.infura.io/v3/099fc58e0de9451d80b18d7c74caa7c1".to_string()),
            alkane_factory_id: env::var("ALKANE_FACTORY_ID").ok(),
        })
    }
}
