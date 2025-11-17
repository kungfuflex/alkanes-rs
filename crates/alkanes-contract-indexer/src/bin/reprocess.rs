use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "reprocess", about = "Force re-process a specific block height")] 
struct Cli {
    /// Block height to re-process
    #[arg(long)]
    height: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).init();

    let cli = Cli::parse();

    // Load config and connect DB
    let cfg = alkanes_contract_indexer::config::AppConfig::from_env()?;
    let pool = alkanes_contract_indexer::db::connect(&cfg.database_url, 5).await?;

    // Build provider
    let provider = alkanes_contract_indexer::provider::build_provider(
        cfg.bitcoin_rpc_url.clone(),
        cfg.sandshrew_rpc_url.clone(),
        cfg.esplora_url.clone(),
        cfg.network_provider.clone(),
    ).await?;

    // Construct pipeline
    let pipeline = alkanes_contract_indexer::pipeline::Pipeline::new(
        pool.clone(),
        cfg.factory_block_id.clone(),
        cfg.factory_tx_id.clone(),
    );

    info!(height = cli.height, "reprocessing block (forced)");
    pipeline.process_block_sequential(&provider, alkanes_contract_indexer::pipeline::BlockContext { height: cli.height, emit_publish: false }).await?;
    info!(height = cli.height, "block reprocessed successfully");
    Ok(())
}


