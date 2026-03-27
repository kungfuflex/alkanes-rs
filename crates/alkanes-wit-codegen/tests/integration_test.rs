use std::path::Path;

/// Helper to write WIT + TOML to a temp dir and return paths.
fn write_test_files(wit_content: &str, toml_content: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("alkanes_wit_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let wit_path = dir.join("test.wit");
    let toml_path = dir.join("alkanes.toml");
    std::fs::write(&wit_path, wit_content).unwrap();
    std::fs::write(&toml_path, toml_content).unwrap();
    (wit_path, toml_path)
}

#[test]
fn test_owned_token_codegen() {
    let wit_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../alkanes/alkanes-std-owned-token-wit/owned-token.wit");
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../alkanes/alkanes-std-owned-token-wit/alkanes.toml");

    // Parse
    let ir = alkanes_wit_parser::parse(&wit_path, &manifest_path).unwrap();

    assert_eq!(ir.name, "OwnedToken");
    assert_eq!(ir.methods.len(), 8);

    // Check specific methods
    let init = ir.methods.iter().find(|m| m.rust_name == "initialize").unwrap();
    assert_eq!(init.opcode, 0);
    assert_eq!(init.params.len(), 2);
    assert!(!init.is_view);

    let get_name = ir.methods.iter().find(|m| m.rust_name == "get_name").unwrap();
    assert_eq!(get_name.opcode, 99);
    assert_eq!(get_name.params.len(), 0);
    assert!(get_name.is_view);
    assert!(matches!(get_name.return_type, alkanes_wit_parser::AlkaneReturnType::Typed(alkanes_wit_parser::AlkaneType::String)));

    let get_data = ir.methods.iter().find(|m| m.rust_name == "get_data").unwrap();
    assert_eq!(get_data.opcode, 1000);
    assert!(matches!(get_data.return_type, alkanes_wit_parser::AlkaneReturnType::Typed(alkanes_wit_parser::AlkaneType::Bytes)));

    // Generate code
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();

    // Verify the generated code contains expected elements
    let code = &generated.module_code;
    assert!(code.contains("OwnedTokenMessage"), "should contain message enum");
    assert!(code.contains("OwnedTokenInterface"), "should contain trait");
    assert!(code.contains("__execute"), "should contain __execute entry point");
    assert!(code.contains("__meta"), "should contain __meta entry point");
    assert!(code.contains("from_opcode"), "should contain from_opcode");
    assert!(code.contains("fn initialize"), "should contain initialize method");
    assert!(code.contains("fn get_name"), "should contain get_name method");
    assert!(code.contains("fn mint"), "should contain mint method");
    assert!(code.contains("export_abi"), "should contain ABI export");

    // Verify ABI JSON structure (quote uses escaped quotes in string literals)
    assert!(code.contains("OwnedToken"), "ABI should reference the contract name");
    assert!(code.contains("__export_abi"), "should have ABI export function");

    // Print for inspection
    println!("=== Generated code length: {} bytes ===", code.len());
}

#[test]
fn test_codegen_writes_to_file() {
    let wit_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../alkanes/alkanes-std-owned-token-wit/owned-token.wit");
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../alkanes/alkanes-std-owned-token-wit/alkanes.toml");

    let ir = alkanes_wit_parser::parse(&wit_path, &manifest_path).unwrap();

    let output_path = std::env::temp_dir().join("alkanes_wit_test_generated.rs");
    alkanes_wit_codegen::generate_to_file(&ir, &output_path).unwrap();

    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("OwnedTokenMessage"));
    assert!(content.contains("OwnedTokenInterface"));

    // Cleanup
    let _ = std::fs::remove_file(&output_path);
}
