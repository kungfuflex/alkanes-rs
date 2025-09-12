use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to input WASM file
    #[arg(short, long)]
    input: PathBuf,

    /// Path to output _build.rs file
    #[arg(short, long)]
    output: PathBuf,
}

use alkanes_build::build;

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    build(args.input, args.output)
}
