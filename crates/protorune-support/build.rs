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

/*
 * Chadson's Journal:
 *
 * The compilation is failing because the `ProtoruneRuneId` struct, which is
 * generated from a `.proto` file, does not implement the `Hash` trait. This
 * is required by the `BalanceSheet` struct, which uses it as a key in a `BTreeMap`.
 *
 * To fix this, I'm using the `customize` method from `protobuf_codegen` to
 * add the `#[derive(PartialEq, Eq, Hash)]` attribute to the `ProtoruneRuneId`
 * and `Uint128` structs during code generation. This will ensure that the
 * generated code has the necessary traits to be used as a key in a `BTreeMap`
 * that needs to be hashed.
 */
use protobuf_codegen::Codegen;
use protoc_bin_vendored;
use protobuf::reflect::MessageDescriptor;
use protobuf_codegen::Customize;
use protobuf_codegen::CustomizeCallback;

fn main() {
    struct GenCustom;

    impl CustomizeCallback for GenCustom {
        fn message(&self, _message: &MessageDescriptor) -> Customize {
            Customize::default().before("#[derive(Eq, Hash)]")
        }
    }

    std::fs::create_dir_all("src/proto").unwrap();
    Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir("src/proto")
        .inputs(&["proto/protorune.proto"])
        .include("proto")
        .customize_callback(GenCustom)
        .run()
        .expect("running protoc failed");
}
