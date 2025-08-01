fn main() {
    prost_build::compile_protos(&["proto/protorune.proto"], &["proto/"]).unwrap();
}
