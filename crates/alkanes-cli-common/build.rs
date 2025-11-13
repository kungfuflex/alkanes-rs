fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();
    
    // Add serde derives for all types
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    
    // Compile the proto files
    config
        .compile_protos(
            &["src/proto/alkanes.proto", "src/proto/protorune.proto"],
            &["src/proto/"]
        )?;
    
    Ok(())
}
