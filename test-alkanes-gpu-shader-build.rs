use anyhow::Result;
use spirv_builder::{MetadataPrintout, SpirvBuilder};
use std::path::Path;

fn main() -> Result<()> {
    println!("Testing alkanes-gpu-shader SPIR-V compilation...");
    
    let shader_crate_path = Path::new("./crates/alkanes-gpu-shader");
    
    let result = SpirvBuilder::new(shader_crate_path, "spirv-unknown-spv1.3")
        .print_metadata(MetadataPrintout::Full)
        .capability(spirv_builder::Capability::Int8)
        .capability(spirv_builder::Capability::Int64)
        .capability(spirv_builder::Capability::Int16)
        .capability(spirv_builder::Capability::StorageBuffer16BitAccess)
        .build();
    
    match result {
        Ok(compile_result) => {
            println!("SUCCESS: alkanes-gpu-shader compiled to SPIR-V!");
            println!("Compile result: {:?}", compile_result);
        }
        Err(e) => {
            println!("FAILED: SPIR-V compilation failed");
            println!("Error: {:?}", e);
            println!("Error display: {}", e);
            
            // Try to get more detailed error information
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