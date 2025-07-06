use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = SpirvBuilder::new("../standalone-spirv-shader", "spirv-unknown-spv1.3")
        .multimodule(true)
        .print_metadata(MetadataPrintout::DependencyOnly)
        .build()?;
    
    println!("SPIR-V build successful!");
    println!("Result: {:?}", result);
    Ok(())
}