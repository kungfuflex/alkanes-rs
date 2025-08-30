use anyhow::{Context, Result};
use clap::Parser;
use rocksdb::{Options, DB};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the first database
    #[clap(short, long, value_parser)]
    db1: PathBuf,

    /// Path to the second database
    #[clap(short, long, value_parser)]
    db2: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let _db1 = DB::open_for_read_only(&Options::default(), &args.db1, false)
        .with_context(|| format!("Failed to open db1 at {:?}", args.db1))?;

    let _db2 = DB::open_for_read_only(&Options::default(), &args.db2, false)
        .with_context(|| format!("Failed to open db2 at {:?}", args.db2))?;

    println!("Successfully opened both databases.");
    println!("Comparing databases...");

    // Comparison logic will go here

    Ok(())
}
