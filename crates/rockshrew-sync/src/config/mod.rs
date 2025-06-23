//! Configuration management for Metashrew indexers
//!
//! This module provides unified configuration management that can be used
//! across different implementations to reduce code duplication.

pub mod database;
pub mod rpc;
pub mod server;
pub mod sync;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

// Re-export configuration types
pub use database::DatabaseConfig;
pub use rpc::RpcConfig;
pub use server::ServerConfig;
pub use sync::SyncConfig as GenericSyncConfig;

/// Main configuration structure that combines all sub-configurations
#[derive(Debug, Clone)]
pub struct MetashrewConfig {
    pub database: DatabaseConfig,
    pub rpc: RpcConfig,
    pub server: ServerConfig,
    pub sync: GenericSyncConfig,
    pub snapshot: Option<SnapshotConfig>,
}

/// Snapshot configuration
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub directory: PathBuf,
    pub interval: u32,
    pub max_snapshots: u32,
    pub compression_level: u32,
    pub reorg_buffer_size: u32,
}

/// Command line arguments structure (can be used as a base for specific implementations)
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct BaseArgs {
    #[arg(long)]
    pub daemon_rpc_url: String,
    
    #[arg(long, required_unless_present = "repo")]
    pub indexer: Option<PathBuf>,
    
    #[arg(long)]
    pub db_path: PathBuf,
    
    #[arg(long)]
    pub start_block: Option<u32>,
    
    #[arg(long)]
    pub auth: Option<String>,
    
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: String,
    
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub port: u16,
    
    #[arg(long)]
    pub label: Option<String>,
    
    #[arg(long)]
    pub exit_at: Option<u32>,
    
    #[arg(
        long,
        help = "Size of the processing pipeline (default: auto-determined based on CPU cores)"
    )]
    pub pipeline_size: Option<usize>,
    
    #[arg(
        long,
        help = "CORS allowed origins (e.g., '*' for all origins, or specific domains)"
    )]
    pub cors: Option<String>,
    
    #[arg(long, help = "Directory to store snapshots for remote sync")]
    pub snapshot_directory: Option<PathBuf>,
    
    #[arg(
        long,
        help = "Interval in blocks to create snapshots (e.g., 1000)",
        default_value_t = 1000
    )]
    pub snapshot_interval: u32,
    
    #[arg(long, help = "URL to a remote snapshot repository to sync from")]
    pub repo: Option<String>,
    
    #[arg(long, help = "Maximum reorg depth to handle", default_value_t = 100)]
    pub max_reorg_depth: u32,
    
    #[arg(
        long,
        help = "Reorg check threshold - only check for reorgs when within this many blocks of tip",
        default_value_t = 6
    )]
    pub reorg_check_threshold: u32,
}

impl MetashrewConfig {
    /// Create configuration from command line arguments
    pub fn from_args(args: BaseArgs) -> Self {
        let database = DatabaseConfig {
            path: args.db_path,
            optimize_for_performance: true,
        };

        let rpc = RpcConfig {
            url: args.daemon_rpc_url,
            auth: args.auth,
            bypass_ssl: false, // Can be determined from URL parsing
            tunnel_config: None, // Can be determined from URL parsing
        };

        let server = ServerConfig {
            host: args.host,
            port: args.port,
            cors: args.cors,
        };

        let sync = GenericSyncConfig {
            start_block: args.start_block.unwrap_or(0),
            exit_at: args.exit_at,
            pipeline_size: args.pipeline_size,
            max_reorg_depth: args.max_reorg_depth,
            reorg_check_threshold: args.reorg_check_threshold,
        };

        let snapshot = args.snapshot_directory.map(|dir| SnapshotConfig {
            directory: dir,
            interval: args.snapshot_interval,
            max_snapshots: 10,
            compression_level: 6,
            reorg_buffer_size: 100,
        });

        Self {
            database,
            rpc,
            server,
            sync,
            snapshot,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.database.validate()?;
        self.rpc.validate()?;
        self.server.validate()?;
        self.sync.validate()?;
        
        if let Some(snapshot) = &self.snapshot {
            snapshot.validate()?;
        }
        
        Ok(())
    }
}

impl SnapshotConfig {
    pub fn validate(&self) -> Result<()> {
        if self.interval == 0 {
            return Err(anyhow::anyhow!("Snapshot interval must be greater than 0"));
        }
        
        if self.max_snapshots == 0 {
            return Err(anyhow::anyhow!("Max snapshots must be greater than 0"));
        }
        
        if self.compression_level > 22 {
            return Err(anyhow::anyhow!("Compression level must be between 0 and 22"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_creation() {
        let args = BaseArgs {
            daemon_rpc_url: "http://localhost:8332".to_string(),
            indexer: Some(PathBuf::from("test.wasm")),
            db_path: PathBuf::from("/tmp/test_db"),
            start_block: Some(100),
            auth: None,
            host: "127.0.0.1".to_string(),
            port: 8080,
            label: None,
            exit_at: None,
            pipeline_size: None,
            cors: None,
            snapshot_directory: None,
            snapshot_interval: 1000,
            repo: None,
            max_reorg_depth: 100,
            reorg_check_threshold: 6,
        };

        let config = MetashrewConfig::from_args(args);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_snapshot_config_validation() {
        let mut config = SnapshotConfig {
            directory: PathBuf::from("/tmp/snapshots"),
            interval: 0, // Invalid
            max_snapshots: 10,
            compression_level: 6,
            reorg_buffer_size: 100,
        };

        assert!(config.validate().is_err());

        config.interval = 1000;
        assert!(config.validate().is_ok());
    }
}