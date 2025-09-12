use std::fs;
use std::path::PathBuf;

pub fn build(input: PathBuf, output: PathBuf) -> std::io::Result<()> {
    // Read WASM file bytes
    let wasm_bytes = fs::read(&input)?;

    // Convert to hex string
    let hex_string = hex::encode(&wasm_bytes);

    // Generate build.rs content
    let build_content = format!(
        "use hex_lit::hex;\n#[allow(long_running_const_eval)]\npub fn get_bytes() -> Vec<u8> {{ (&hex!(\"{}\")).to_vec() }}",
        hex_string
    );

    // Write output file
    fs::write(&output, build_content)?;

    println!(
        "Successfully converted {} to {}",
        input.display(),
        output.display()
    );

    Ok(())
}