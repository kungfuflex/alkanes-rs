use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Build the metashrew-minimal WASM
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .arg("-p")
        .arg("metashrew-minimal")
        .arg("--target-dir")
        .arg(format!("{}/wasm", out_dir))
        .status()
        .expect("Failed to build metashrew-minimal");

    if !status.success() {
        panic!("Failed to build metashrew-minimal WASM module");
    }

    let wasm_path = format!(
        "{}/wasm/wasm32-unknown-unknown/release/metashrew_minimal.wasm",
        out_dir
    );

    println!("cargo:rustc-env=METASHREW_MINIMAL_WASM_PATH={}", wasm_path);
    println!("cargo:rerun-if-changed=../metashrew-minimal/src/lib.rs");
}
