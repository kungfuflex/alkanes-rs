use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/alkanes.proto");
    protobuf_codegen::Codegen::new()
        .pure()
        .cargo_out_dir("proto")
        .input("src/alkanes.proto")
        .include("src")
        .run_from_script();
}