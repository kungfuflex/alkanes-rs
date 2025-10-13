fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config
        .compile_protos(&["proto/alkanes.proto"], &["proto/"])
        .unwrap();
    Ok(())
}
