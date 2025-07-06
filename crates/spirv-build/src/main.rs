extern crate spirv_builder;
use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() {
    let result = SpirvBuilder::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../spirv-test"),
        "spirv-unknown-spv1.3",
    )
    .print_metadata(MetadataPrintout::DependencyOnly)
    .multimodule(true)
    .build()
    .unwrap();
    println!("{result:#?}");
}
