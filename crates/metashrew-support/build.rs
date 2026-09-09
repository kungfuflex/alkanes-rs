fn main() {
    prost_build::compile_protos(&["proto/metashrew.proto"], &["proto/"]).unwrap();
}
