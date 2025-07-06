extern crate spirv_builder;
use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() {
    let result = SpirvBuilder::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../alkanes-gpu-shader"),
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
