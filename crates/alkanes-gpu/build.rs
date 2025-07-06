//! Build script for alkanes-gpu
//! 
//! This script compiles the alkanes-gpu-shader crate to SPIR-V using spirv-builder.
//! The resulting SPIR-V binary can be loaded by Vulkan for GPU execution.

use anyhow::Result;
use spirv_builder::{MetadataPrintout, SpirvBuilder};
use std::env;
use std::path::Path;

fn main() -> Result<()> {
    // Only build SPIR-V if we're targeting a native platform
    // Skip SPIR-V compilation when building for WASM or other non-native targets
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("wasm") || target.contains("unknown") {
        println!("cargo:warning=alkanes-gpu@{}: Skipping SPIR-V compilation for target: {}", 
                 env::var("CARGO_PKG_VERSION").unwrap_or_default(), target);
        return Ok(());
    }
    
    // Check if we should build SPIR-V shaders
    let build_spirv = env::var("ALKANES_BUILD_SPIRV").unwrap_or_default();
    if build_spirv != "1" && build_spirv.to_lowercase() != "true" {
        println!("cargo:warning=alkanes-gpu@{}: Building CPU target (no SPIR-V compilation)", 
                 env::var("CARGO_PKG_VERSION").unwrap_or_default());
        println!("cargo:warning=Set ALKANES_BUILD_SPIRV=1 to enable SPIR-V compilation");
        return Ok(());
    }
    
    println!("cargo:warning=alkanes-gpu@{}: Building SPIR-V shaders for GPU execution", 
             env::var("CARGO_PKG_VERSION").unwrap_or_default());
    
    // Path to the shader crate
    let shader_crate_path = Path::new("../alkanes-gpu-shader");
    
    // Verify shader crate exists
    if !shader_crate_path.exists() {
        return Err(anyhow::anyhow!(
            "Shader crate not found at: {}. Please ensure alkanes-gpu-shader crate exists.",
            shader_crate_path.display()
        ));
    }
    
    // Build the shader crate to SPIR-V
    let result = SpirvBuilder::new(shader_crate_path, "spirv-unknown-vulkan1.1")
        .print_metadata(MetadataPrintout::Full)
        .capability(spirv_builder::Capability::Int64)
        .capability(spirv_builder::Capability::Int16)
        .capability(spirv_builder::Capability::StorageBuffer16BitAccess)
        .build();
    
    match result {
        Ok(_) => {
            println!("cargo:warning=Successfully compiled alkanes-gpu-shader to SPIR-V");
            
            // The SPIR-V binary path will be available as an environment variable
            // named after the crate: alkanes_gpu_shader.spv
            if let Ok(spirv_path) = env::var("alkanes_gpu_shader.spv") {
                println!("cargo:warning=SPIR-V binary available at: {}", spirv_path);
                
                // Make the SPIR-V path available to the main crate
                println!("cargo:rustc-env=ALKANES_GPU_SPIRV_PATH={}", spirv_path);
            }
        }
        Err(e) => {
            // Don't fail the build if SPIR-V compilation fails
            // This allows the crate to still build for CPU-only testing
            println!("cargo:warning=SPIR-V compilation failed: {}", e);
            println!("cargo:warning=Continuing with CPU-only build");
        }
    }
    
    Ok(())
}