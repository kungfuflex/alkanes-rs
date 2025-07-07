extern crate spirv_builder;
use spirv_builder::{MetadataPrintout, SpirvBuilder};
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Default to alkanes-gpu-shader if no argument provided
    let crate_path = if args.len() > 1 {
        &args[1]
    } else {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../alkanes-gpu-shader")
    };
    
    // Convert relative paths to absolute paths from the workspace root
    let absolute_path = if Path::new(crate_path).is_absolute() {
        crate_path.to_string()
    } else if crate_path.starts_with("../") {
        // Handle paths relative to spirv-build directory (go up to workspace root)
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let relative_part = &crate_path[3..]; // Remove "../"
        workspace_root.join(relative_part).to_string_lossy().to_string()
    } else {
        // Handle paths relative to spirv-build directory
        format!("{}/{}", env!("CARGO_MANIFEST_DIR"), crate_path)
    };
    
    println!("Building SPIR-V crate: {}", absolute_path);
    
    let result = SpirvBuilder::new(
        &absolute_path,
        "spirv-unknown-spv1.3",
    )
    .print_metadata(MetadataPrintout::DependencyOnly)
    .multimodule(true)
    .capability(spirv_builder::Capability::Int8)
    .capability(spirv_builder::Capability::Int64)
    .capability(spirv_builder::Capability::Int16)
    .capability(spirv_builder::Capability::StorageBuffer16BitAccess)
    .build()
    .unwrap();
    println!("{result:#?}");
}
