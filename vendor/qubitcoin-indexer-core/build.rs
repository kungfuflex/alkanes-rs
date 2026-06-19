fn main() {
    prost_build::compile_protos(&["proto/metashrew.proto"], &["proto/"])
        .expect("failed to compile protobuf");
}
