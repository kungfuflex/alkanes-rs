// This build script is responsible for compiling Protocol Buffers (`.proto` files)
// into Rust code and customizing the generated structs.
//
// This implementation is based on the `customize-serde` example from the `rust-protobuf`
// repository, which can be found at:
// ./reference/rust-protobuf/protobuf-examples/customize-serde/build.rs
//
// The key customizations are:
// 1. Adding `#[derive(serde::Serialize, serde::Deserialize, ...)]` to all messages.
// 2. Adding `#[serde(skip)]` to the internal `special_fields` member of each message,
//    which is not serializable and was causing compilation errors.

use protobuf_codegen::Codegen;
use protoc_bin_vendored;

fn main() {
    std::fs::create_dir_all("src/proto").unwrap();
    Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir("src/proto")
        .inputs(&["proto/protorune.proto"])
        .include("proto")
        .run()
        .expect("running protoc failed");
}
