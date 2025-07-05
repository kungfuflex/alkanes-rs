//! Build script for alkanes-gpu crate
//!
//! This build script handles SPIR-V compilation when building for GPU targets.
//! It uses spirv-builder to compile the Rust code to SPIR-V bytecode that
//! can be executed on Vulkan-compatible GPUs.

use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let out_dir = env::var("OUT_DIR").unwrap_or_default();
    
    // Tell cargo to rerun if any source files change
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Check if we should build SPIR-V
    let should_build_spirv = env::var("CARGO_FEATURE_SPIRV").is_ok() && env::var("ALKANES_BUILD_SPIRV").is_ok();
    
    if should_build_spirv {
        println!("cargo:warning=SPIR-V compilation requested but not implemented yet");
        println!("cargo:warning=Set ALKANES_BUILD_SPIRV=1 to enable SPIR-V compilation");
        
        // TODO: Implement actual spirv-builder integration
        // This would require setting up a separate shader crate
        // and using spirv-builder to compile it
        
        // Set SPIR-V specific configuration
        println!("cargo:rustc-cfg=spirv_enabled");
        if target.contains("vulkan") {
            println!("cargo:rustc-cfg=vulkan");
        }
    } else {
        println!("cargo:warning=Building CPU target (no SPIR-V compilation)");
        println!("cargo:rustc-cfg=cpu_build");
    }
    
    // Set up output directory for SPIR-V binaries
    let spirv_dir = PathBuf::from(&out_dir).join("spirv");
    if let Err(_) = std::fs::create_dir_all(&spirv_dir) {
        // Ignore errors creating directory
    }
    
    println!("cargo:rustc-env=SPIRV_OUT_DIR={}", spirv_dir.display());
}