use protobuf_codegen;
use protoc_bin_vendored;
fn main() {
    protobuf_codegen::Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .out_dir("src/proto")
        .inputs(&["src/proto/protorune.proto", "src/proto/alkanes.proto"])
        .include("src/proto")
        .run()
        .expect("running protoc failed");
}
