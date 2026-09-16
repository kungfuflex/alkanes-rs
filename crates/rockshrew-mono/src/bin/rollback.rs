//! Rockshrew Rollback Tool
//!
//! This binary rolls back a rockshrew database to a specific block height.
//! It removes all indexed data for blocks above the target height and updates
//! the indexed height marker, allowing rockshrew-mono to resume indexing from
//! that point.
//!
//! ## Usage
//!
//! ```bash
//! rockshrew-rollback --db-path /data/.metashrew-v9.0.2-v2.1.6 --height 925000
//! ```
//!
//! After running this tool, you can start rockshrew-mono and it will resume
//! indexing from block 925001.
//!
//! ## Safety
//!
//! - Always backup your database before running a rollback
//! - Ensure rockshrew-mono is NOT running when performing a rollback
//! - The rollback operation modifies the database in-place

use anyhow::{Context, Result};
use clap::Parser;
use log::{info, warn};
use rocksdb::{Options, DB};
use std::path::PathBuf;
use std::sync::Arc;

use metashrew_runtime::rollback::rollback_smt_data;
use rockshrew_runtime::storage_adapter::RocksDBStorageAdapter;

#[derive(Parser, Debug)]
#[command(
    name = "rockshrew-rollback",
    about = "Roll back rockshrew database to a specific block height",
    long_about = "This tool rolls back a rockshrew database to a specific block height by removing \
                  all indexed data for blocks above the target height. After rollback, rockshrew-mono \
                  can be restarted and will resume indexing from the rollback height."
)]
struct Args {
    /// Path to the RocksDB database directory
    #[arg(long, value_name = "PATH")]
    db_path: PathBuf,

    /// Target block height to roll back to
    #[arg(long, value_name = "HEIGHT")]
    height: u32,

    /// Skip confirmation prompt (dangerous!)
    #[arg(long)]
    yes: bool,
}

fn main() -> Result<()> {
    env_logger::builder()
        .format_timestamp_secs()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    // Validate database path exists
    if !args.db_path.exists() {
        anyhow::bail!("Database path does not exist: {}", args.db_path.display());
    }

    info!("=== Rockshrew Database Rollback Tool ===");
    info!("Database path: {}", args.db_path.display());
    info!("Target height: {}", args.height);

    // Open database in read-only mode first to check current state
    info!("Opening database to check current state...");
    let db = DB::open_for_read_only(&Options::default(), &args.db_path, false)
        .context("Failed to open database for reading")?;

    let height_key = b"__INTERNAL/height";
    let current_height = match db.get(height_key)? {
        Some(bytes) if bytes.len() >= 4 => {
            let height_bytes: [u8; 4] = bytes[..4].try_into().unwrap();
            u32::from_le_bytes(height_bytes)
        }
        Some(_) => {
            warn!("Invalid height data in database, assuming height 0");
            0
        }
        None => {
            warn!("No height marker found in database, assuming height 0");
            0
        }
    };

    drop(db); // Close read-only handle

    info!("Current indexed height: {}", current_height);

    // Validate rollback height
    if args.height >= current_height {
        anyhow::bail!(
            "Rollback height ({}) must be less than current height ({})",
            args.height,
            current_height
        );
    }

    let blocks_to_remove = current_height - args.height;
    warn!("This will REMOVE data for {} blocks ({} to {})",
        blocks_to_remove,
        args.height + 1,
        current_height
    );

    // Confirmation prompt
    if !args.yes {
        println!("\n⚠️  WARNING: This operation will modify the database in-place!");
        println!("⚠️  Make sure rockshrew-mono is NOT running!");
        println!("⚠️  Consider backing up your database first!");
        println!("\nType 'yes' to proceed with rollback: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read confirmation")?;

        if input.trim().to_lowercase() != "yes" {
            println!("Rollback cancelled.");
            return Ok(());
        }
    }

    info!("Opening database for rollback...");
    let db = Arc::new(DB::open_default(&args.db_path)
        .context("Failed to open database. Is rockshrew-mono still running?")?);

    // Create storage adapter for rollback
    let mut adapter = RocksDBStorageAdapter::new(db.clone());

    // Perform the rollback
    info!("Starting rollback operation...");
    rollback_smt_data(&mut adapter, args.height, current_height)
        .context("Rollback operation failed")?;

    // Update the indexed height marker
    info!("Updating indexed height marker to {}...", args.height);
    let height_bytes = args.height.to_le_bytes();
    db.put(height_key, &height_bytes)
        .context("Failed to update height marker")?;

    // Verify the rollback
    let new_height = match db.get(height_key)? {
        Some(bytes) if bytes.len() >= 4 => {
            let height_bytes: [u8; 4] = bytes[..4].try_into().unwrap();
            u32::from_le_bytes(height_bytes)
        }
        _ => 0,
    };

    if new_height != args.height {
        anyhow::bail!(
            "Verification failed! Expected height {} but found {}",
            args.height,
            new_height
        );
    }

    info!("✅ Rollback completed successfully!");
    info!("Database has been rolled back from height {} to height {}", current_height, args.height);
    info!("You can now restart rockshrew-mono to resume indexing from block {}", args.height + 1);

    Ok(())
}
