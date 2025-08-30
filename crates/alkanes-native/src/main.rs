use anyhow::Result;
use clap::{Parser, ValueEnum};
use adapters::{NativeRuntimeAdapter, RpcAdapter, RocksDBAdapter};
use metashrew_sync::{MetashrewSync, SyncConfig, SyncEngine};

mod adapters;
mod shred_host;

#[derive(ValueEnum, Debug, Clone)]
pub enum Network {
    Mainnet,
    Regtest,
    Signet,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "http://localhost:18888")]
    pub daemon_rpc_url: String,
    #[arg(long)]
    pub db_path: std::path::PathBuf,
    #[arg(long, value_enum, default_value_t = Network::Regtest)]
    pub network: Network,
    #[arg(long)]
    pub start_block: Option<u32>,
    #[arg(long)]
    pub exit_at: Option<u32>,
    #[arg(long)]
    pub pipeline_size: Option<usize>,
    #[arg(long, default_value_t = 100)]
    pub max_reorg_depth: u32,
    #[arg(long, default_value_t = 6)]
    pub reorg_check_threshold: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let node_adapter = RpcAdapter::new(&args.daemon_rpc_url, "", "", args.network.clone())?;
    let storage_adapter = RocksDBAdapter::new(&args.db_path.to_string_lossy())?;
    shred_host::set_storage_adapter(storage_adapter.clone());
    let runtime_adapter = NativeRuntimeAdapter;

    let config = SyncConfig {
        start_block: args.start_block.unwrap_or(0),
        exit_at: args.exit_at,
        pipeline_size: args.pipeline_size,
        max_reorg_depth: args.max_reorg_depth,
        reorg_check_threshold: args.reorg_check_threshold,
    };

    let mut sync_engine = MetashrewSync::new(
        node_adapter,
        storage_adapter,
        runtime_adapter,
        config,
    );

    sync_engine.start().await?;

    Ok(())
}