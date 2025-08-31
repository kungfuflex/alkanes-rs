use anyhow::Result;
use rockshrew_mono::{run_prod, Args};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    run_prod(args).await
}