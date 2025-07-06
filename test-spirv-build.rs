//! Test script to debug SPIR-V compilation issues
//! This will help us understand what's going wrong with spirv-builder

use spirv_builder::{MetadataPrintout, SpirvBuilder};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing SPIR-V compilation...");
    
    // Path to the shader crate
    let shader_crate_path = Path::new("crates/alkanes-gpu-shader");
    
    println!("Shader crate path: {}", shader_crate_path.display());
    println!("Shader crate exists: {}", shader_crate_path.exists());
    
    if !shader_crate_path.exists() {
        return Err("Shader crate not found".into());
    }
    
    println!("Attempting SPIR-V build...");
    
    let result = SpirvBuilder::new(shader_crate_path, "spirv-unknown-vulkan1.1")
        .print_metadata(MetadataPrintout::Full)
        .capability(spirv_builder::Capability::Int64)
        .capability(spirv_builder::Capability::Int16)
        .capability(spirv_builder::Capability::StorageBuffer16BitAccess)
        .build();
    
    match result {
        Ok(compile_result) => {
            println!("SUCCESS: SPIR-V compilation succeeded!");
            println!("Compile result: {:?}", compile_result);
        }
        Err(e) => {
            println!("ERROR: SPIR-V compilation failed");
            println!("Error: {:?}", e);
            println!("Error display: {}", e);
            
            // Try to get more details about the error
            if let Some(source) = e.source() {
                println!("Error source: {:?}", source);
            }
            
            return Err(e.into());
        }
    }
    
    Ok(())
}