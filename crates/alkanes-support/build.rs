fn main() {
    prost_build::compile_protos(&["proto/alkanes.proto"], &["proto/"]).unwrap();
}
