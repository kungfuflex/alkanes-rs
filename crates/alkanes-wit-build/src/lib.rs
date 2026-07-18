use anyhow::Result;
use std::path::Path;

/// Generate Rust code from a WIT file and alkanes.toml manifest.
///
/// Call this from your contract's `build.rs`:
/// ```ignore
/// fn main() {
///     alkanes_wit_build::generate(
///         "src/contract.wit",
///         "src/alkanes.toml",
///         "src/generated.rs",
///     ).unwrap();
/// }
/// ```
pub fn generate(wit_path: &str, manifest_path: &str, output_path: &str) -> Result<()> {
    let wit = Path::new(wit_path);
    let manifest = Path::new(manifest_path);
    let output = Path::new(output_path);

    let ir = alkanes_wit_parser::parse(wit, manifest)?;
    alkanes_wit_codegen::generate_to_file(&ir, output)?;

    // Tell cargo to re-run if the WIT file or manifest changes
    println!("cargo:rerun-if-changed={}", wit_path);
    println!("cargo:rerun-if-changed={}", manifest_path);

    Ok(())
}

/// Generate Rust code from a WIT directory and alkanes.toml manifest.
pub fn generate_from_dir(wit_dir: &str, manifest_path: &str, output_path: &str) -> Result<()> {
    generate(wit_dir, manifest_path, output_path)
}
