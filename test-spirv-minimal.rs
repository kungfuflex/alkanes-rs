//! Minimal test for SPIR-V compilation

use spirv_builder::{MetadataPrintout, SpirvBuilder};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing minimal SPIR-V compilation...");
    
    let shader_crate_path = Path::new("test-minimal-shader");
    
    println!("Shader crate path: {}", shader_crate_path.display());
    println!("Shader crate exists: {}", shader_crate_path.exists());
    
    if !shader_crate_path.exists() {
        return Err("Shader crate not found".into());
    }
    
    println!("Attempting minimal SPIR-V build...");
    
    let result = SpirvBuilder::new(shader_crate_path, "spirv-unknown-vulkan1.1")
        .print_metadata(MetadataPrintout::Full)
        .build();
    
    match result {
        Ok(compile_result) => {
            println!("SUCCESS: Minimal SPIR-V compilation succeeded!");
            println!("Compile result: {:?}", compile_result);
        }
        Err(e) => {
            println!("ERROR: Minimal SPIR-V compilation failed");
            println!("Error: {:?}", e);
            println!("Error display: {}", e);
            
            // Try to get more details about the error
            let mut current_error: &dyn std::error::Error = &e;
            let mut error_chain = Vec::new();
            
            while let Some(source) = current_error.source() {
                error_chain.push(format!("{}", source));
                current_error = source;
            }
            
            if !error_chain.is_empty() {
                println!("Error chain:");
                for (i, err) in error_chain.iter().enumerate() {
                    println!("  {}: {}", i + 1, err);
                }
            }
            
            return Err(e.into());
        }
    }
    
    Ok(())
}