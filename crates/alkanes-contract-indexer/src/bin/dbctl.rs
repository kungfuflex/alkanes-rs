use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "dbctl", about = "Database management CLI for alkanes-contract-indexer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create or update the DB schema
    Push,
    /// Drop all tables and recreate the schema
    Reset,
    /// Drop all tables without re-pushing the schema
    Drop,
    /// Run sqlx migrations from the migrations/ directory
    Migrate,
    /// Set progress so the next run starts at a given block height
    ///
    /// Usage:
    ///   dbctl reset-progress --height <H> [--block-hash <HASH>]
    /// Behavior:
    ///   - H > 0: sets position to H-1 so the next run starts at H
    ///   - H = 0: clears the position; to start from 0, run the indexer without START_HEIGHT
    ResetProgress {
        /// Height to start from on the next run (H>0 starts at H; H=0 clears progress)
        #[arg(long, default_value_t = 0)]
        height: u64,
        /// Block hash of the block at height-1 (optional, defaults to empty if not provided)
        #[arg(long, default_value = "")]
        block_hash: String,
    },
    /// Delete all indexed data for a specific block height
    PurgeBlock {
        /// Block height to purge
        #[arg(long)]
        height: i32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(env_filter).init();

    let cli = Cli::parse();
    let cfg = alkanes_contract_indexer::config::AppConfig::from_env()?;
    let pool = alkanes_contract_indexer::db::connect(&cfg.database_url, 5).await?;
    match cli.command {
        Commands::Push => {
            alkanes_contract_indexer::schema::push_schema(&pool).await?;
            info!("Schema pushed successfully");
        }
        Commands::Reset => {
            alkanes_contract_indexer::schema::reset_schema(&pool).await?;
            info!("Schema reset successfully");
        }
        Commands::Drop => {
            alkanes_contract_indexer::schema::drop_all_tables(&pool).await?;
            info!("All tables dropped successfully");
        }
        Commands::Migrate => {
            // Apply migrations in migrations/ folder at crate root
            sqlx::migrate!().run(&pool).await?;
            info!("Migrations applied successfully");
        }
        Commands::ResetProgress { height, block_hash } => {
            // Ensure position table exists
            alkanes_contract_indexer::progress::ensure_position_table(&pool).await?;
            if height == 0 {
                // Clearing the position means: if START_HEIGHT is unset, next run starts at 0; if START_HEIGHT is set, it will start from that value.
                sqlx::query("DELETE FROM indexer_position WHERE id = 1")
                    .execute(&pool)
                    .await?;
                info!("Cleared position (next run depends on START_HEIGHT; unset it to begin at 0)");
            } else {
                let progress = alkanes_contract_indexer::progress::ProgressStore::new(pool.clone());
                // Set position to height-1, so coordinator starts from height regardless of START_HEIGHT
                let last = height - 1;
                progress.set_position(last, &block_hash).await?;
                info!(target_height = height, last_position = last, block_hash = %block_hash, "Configured next run to start from height {}", height);
            }
        }
        Commands::PurgeBlock { height } => {
            // Purge all data for a block height in dependency-safe order inside a single transaction
            let mut tx = pool.begin().await?;

            // Derived event tables (no FKs to transactions)
            sqlx::query(r#"delete from "PoolSwap" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "PoolMint" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "PoolBurn" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "PoolCreation" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "SubfrostWrap" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "SubfrostUnwrap" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;

            // Trace/decoded and clock-in (FKs to AlkaneTransaction)
            sqlx::query(r#"delete from "TraceEvent" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "DecodedProtostone" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "ClockIn" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;

            // Core transactions
            sqlx::query(r#"delete from "AlkaneTransaction" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;

            // Per-block metadata/state
            sqlx::query(r#"delete from "ProcessedBlocks" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "PoolState" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;
            sqlx::query(r#"delete from "ClockInBlockSummary" where "blockHeight" = $1"#).bind(height).execute(&mut *tx).await?;

            tx.commit().await?;
            info!(block_height = height, "Purged all data for blockHeight={}", height);
        }
    }
    Ok(())
}


