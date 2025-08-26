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

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    // Read WASM file bytes
    let wasm_bytes = fs::read(&args.input)?;

    // Convert to hex string
    let hex_string = hex::encode(&wasm_bytes);

    // Generate build.rs content
    let build_content = format!(
        "use hex_lit::hex;\n#[allow(long_running_const_eval)]\n#[allow(deprecated)]\npub fn get_bytes() -> Vec<u8> {{ (&hex!(\"{}\")).to_vec() }}",
        hex_string
    );

    // Write output file
    fs::write(&args.output, build_content)?;

    println!(
        "Successfully converted {} to {}",
        args.input.display(),
        args.output.display()
    );

    Ok(())
}
