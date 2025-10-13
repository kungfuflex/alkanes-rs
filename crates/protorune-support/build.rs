use anyhow::Result;

fn main() -> Result<()> {
    protobuf_codegen::Codegen::new()
        .pure()
        .cargo_out_dir("protos")
        .input("./proto/protorune.proto")
        .include("./proto")
        .run_from_script();
    Ok(())
}