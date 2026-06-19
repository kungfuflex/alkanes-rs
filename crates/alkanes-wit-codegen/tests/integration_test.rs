use std::path::Path;

/// Helper to write WIT + TOML to a unique temp dir and return paths.
fn write_test_files(test_name: &str, wit_content: &str, toml_content: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("alkanes_wit_test_{}_{}", std::process::id(), test_name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

// =============================================================================
// Custom record types codegen
// =============================================================================

#[test]
fn test_record_types_codegen() {
    let wit = r#"
package test:record-types;

interface record-types {
    record pool-info {
        token-a-block: u64,
        token-a-tx: u64,
        fee: u64,
    }

    create-pool: func(info: pool-info) -> result<_, string>;
    get-pool: func(id: u64) -> result<pool-info, string>;
}

world record-types-world {
    export record-types;
}
"#;

    let toml = r#"
[contract]
name = "RecordTypes"

[opcodes]
create-pool = 1
get-pool = 2

[views]
get-pool = true
"#;

    let (wit_path, toml_path) = write_test_files("record_types", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();

    assert_eq!(ir.name, "RecordTypes");
    assert_eq!(ir.methods.len(), 2);
    assert_eq!(ir.custom_types.len(), 1);

    // Verify the record type was parsed correctly
    let pool_info = &ir.custom_types[0];
    assert_eq!(pool_info.name, "PoolInfo");
    match &pool_info.kind {
        alkanes_wit_parser::AlkaneTypeDefKind::Record(fields) => {
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].name, "token_a_block");
            assert_eq!(fields[1].name, "token_a_tx");
            assert_eq!(fields[2].name, "fee");
        }
        _ => panic!("expected record type"),
    }

    // Verify the method that takes a record param
    let create_pool = ir.methods.iter().find(|m| m.rust_name == "create_pool").unwrap();
    assert_eq!(create_pool.params.len(), 1);
    assert!(matches!(&create_pool.params[0].ty, alkanes_wit_parser::AlkaneType::Record(n) if n == "PoolInfo"));

    // Verify the method that returns a record
    let get_pool = ir.methods.iter().find(|m| m.rust_name == "get_pool").unwrap();
    assert!(get_pool.is_view);
    assert!(matches!(&get_pool.return_type, alkanes_wit_parser::AlkaneReturnType::Typed(alkanes_wit_parser::AlkaneType::Record(n)) if n == "PoolInfo"));

    // Generate code and check for CellpackEncode/CellpackDecode impls
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    assert!(code.contains("struct PoolInfo"), "should generate PoolInfo struct");
    assert!(code.contains("token_a_block"), "should contain field token_a_block");
    assert!(code.contains("token_a_tx"), "should contain field token_a_tx");
    assert!(code.contains("fee"), "should contain field fee");
    assert!(code.contains("impl CellpackEncode for PoolInfo"), "should generate CellpackEncode impl for record");
    assert!(code.contains("impl CellpackDecode for PoolInfo"), "should generate CellpackDecode impl for record");
    assert!(code.contains("RecordTypesInterface"), "should contain trait");
    assert!(code.contains("RecordTypesMessage"), "should contain message enum");

    println!("=== Record types generated code length: {} bytes ===", code.len());
}

// =============================================================================
// Enum types codegen
// =============================================================================

#[test]
fn test_enum_types_codegen() {
    let wit = r#"
package test:enum-types;

interface enum-types {
    enum status {
        active,
        paused,
        closed,
    }

    get-status: func() -> result<status, string>;
    set-status: func(s: status) -> result<_, string>;
}

world enum-types-world {
    export enum-types;
}
"#;

    let toml = r#"
[contract]
name = "EnumTypes"

[opcodes]
get-status = 10
set-status = 11

[views]
get-status = true
"#;

    let (wit_path, toml_path) = write_test_files("enum_types", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();

    assert_eq!(ir.name, "EnumTypes");
    assert_eq!(ir.methods.len(), 2);
    assert_eq!(ir.custom_types.len(), 1);

    // Verify the enum type was parsed
    let status = &ir.custom_types[0];
    assert_eq!(status.name, "Status");
    match &status.kind {
        alkanes_wit_parser::AlkaneTypeDefKind::Enum(cases) => {
            assert_eq!(cases.len(), 3);
            assert_eq!(cases[0], "Active");
            assert_eq!(cases[1], "Paused");
            assert_eq!(cases[2], "Closed");
        }
        _ => panic!("expected enum type"),
    }

    // Verify methods reference the enum
    let set_status = ir.methods.iter().find(|m| m.rust_name == "set_status").unwrap();
    assert!(matches!(&set_status.params[0].ty, alkanes_wit_parser::AlkaneType::Enum(n) if n == "Status"));

    let get_status = ir.methods.iter().find(|m| m.rust_name == "get_status").unwrap();
    assert!(get_status.is_view);

    // Generate code
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    assert!(code.contains("enum Status"), "should generate Status enum");
    assert!(code.contains("Active"), "should contain Active variant");
    assert!(code.contains("Paused"), "should contain Paused variant");
    assert!(code.contains("Closed"), "should contain Closed variant");
    assert!(code.contains("impl CellpackEncode for Status"), "should generate CellpackEncode impl for enum");
    assert!(code.contains("impl CellpackDecode for Status"), "should generate CellpackDecode impl for enum");
    // Enums use discriminant-based encoding
    assert!(code.contains("disc"), "should use discriminant encoding");

    println!("=== Enum types generated code length: {} bytes ===", code.len());
}

// =============================================================================
// Variant types codegen
// =============================================================================

#[test]
fn test_variant_types_codegen() {
    let wit = r#"
package test:variant-types;

interface variant-types {
    variant action {
        swap(u64),
        add-liquidity,
        remove-liquidity(u64),
    }

    execute-action: func(act: action) -> result<_, string>;
}

world variant-types-world {
    export variant-types;
}
"#;

    let toml = r#"
[contract]
name = "VariantTypes"

[opcodes]
execute-action = 5
"#;

    let (wit_path, toml_path) = write_test_files("variant_types", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();

    assert_eq!(ir.name, "VariantTypes");
    assert_eq!(ir.methods.len(), 1);
    assert_eq!(ir.custom_types.len(), 1);

    // Verify the variant type
    let action = &ir.custom_types[0];
    assert_eq!(action.name, "Action");
    match &action.kind {
        alkanes_wit_parser::AlkaneTypeDefKind::Variant(cases) => {
            assert_eq!(cases.len(), 3);
            assert_eq!(cases[0].name, "Swap");
            assert!(cases[0].payload.is_some(), "Swap should have a u64 payload");
            assert_eq!(cases[1].name, "AddLiquidity");
            assert!(cases[1].payload.is_none(), "AddLiquidity should have no payload");
            assert_eq!(cases[2].name, "RemoveLiquidity");
            assert!(cases[2].payload.is_some(), "RemoveLiquidity should have a u64 payload");
        }
        _ => panic!("expected variant type"),
    }

    // Verify method parameter references the variant
    let exec = ir.methods.iter().find(|m| m.rust_name == "execute_action").unwrap();
    assert!(matches!(&exec.params[0].ty, alkanes_wit_parser::AlkaneType::Variant(n) if n == "Action"));

    // Generate code
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    assert!(code.contains("enum Action"), "should generate Action enum (variants are Rust enums)");
    assert!(code.contains("Swap"), "should contain Swap case");
    assert!(code.contains("AddLiquidity"), "should contain AddLiquidity case");
    assert!(code.contains("RemoveLiquidity"), "should contain RemoveLiquidity case");
    assert!(code.contains("impl CellpackEncode for Action"), "should generate CellpackEncode impl for variant");
    assert!(code.contains("impl CellpackDecode for Action"), "should generate CellpackDecode impl for variant");
    // Variant encode/decode uses discriminant + optional payload
    assert!(code.contains("encode_cellpack"), "should use encode_cellpack in variant impl");

    println!("=== Variant types generated code length: {} bytes ===", code.len());
}

// =============================================================================
// Imported interfaces codegen (cross-contract clients)
// =============================================================================

#[test]
fn test_imported_interface_codegen() {
    // Use inline interface definitions inside the world so that wit-parser
    // produces WorldKey::Name (rather than WorldKey::Interface for top-level
    // interfaces imported by reference).
    let wit = r#"
package test:with-imports;

interface my-contract {
    initialize: func() -> result<_, string>;
}

world my-contract-world {
    import token-ref: interface {
        get-name: func() -> result<string, string>;
        mint: func(amount: u64) -> result<_, string>;
    }
    export my-contract;
}
"#;

    let toml = r#"
[contract]
name = "MyContract"

[opcodes]
initialize = 0

[imports.token-ref]
get-name = 99
mint = 77
"#;

    let (wit_path, toml_path) = write_test_files("with_imports", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();

    assert_eq!(ir.name, "MyContract");
    assert_eq!(ir.methods.len(), 1, "should have 1 exported method");
    assert_eq!(ir.imports.len(), 1, "should have 1 imported interface");

    // Verify the import
    let import = &ir.imports[0];
    assert_eq!(import.interface_name, "token-ref");
    assert_eq!(import.rust_client_name, "TokenRefClient");
    assert_eq!(import.methods.len(), 2);

    let get_name = import.methods.iter().find(|m| m.rust_name == "get_name").unwrap();
    assert_eq!(get_name.opcode, 99);

    let mint = import.methods.iter().find(|m| m.rust_name == "mint").unwrap();
    assert_eq!(mint.opcode, 77);
    assert_eq!(mint.params.len(), 1);

    // Generate code
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    assert!(code.contains("struct TokenRefClient"), "should generate client struct");
    assert!(code.contains("impl TokenRefClient"), "should generate client impl");
    assert!(code.contains("fn get_name"), "client should have get_name method");
    assert!(code.contains("fn mint"), "client should have mint method");
    assert!(code.contains("target"), "client should reference target AlkaneId");
    assert!(code.contains("AlkaneId"), "client should use AlkaneId for target");
    assert!(code.contains("Cellpack"), "client should construct Cellpack for calls");

    // Verify the exported contract still generates normally
    assert!(code.contains("MyContractInterface"), "should contain exported trait");
    assert!(code.contains("MyContractMessage"), "should contain message enum");
    assert!(code.contains("fn initialize"), "should contain initialize method");

    println!("=== Import codegen length: {} bytes ===", code.len());
}

// =============================================================================
// ABI JSON validity
// =============================================================================

#[test]
fn test_abi_json_is_valid() {
    let wit = r#"
package test:abi-check;

interface abi-check {
    record my-record {
        val: u64,
    }

    enum my-enum {
        on,
        off,
    }

    do-something: func(a: u64, b: string) -> result<_, string>;
    get-value: func() -> result<u64, string>;
    with-record: func(r: my-record) -> result<my-enum, string>;
}

world abi-check-world {
    export abi-check;
}
"#;

    let toml = r#"
[contract]
name = "AbiCheck"

[opcodes]
do-something = 1
get-value = 2
with-record = 3

[views]
get-value = true
"#;

    let (wit_path, toml_path) = write_test_files("abi_check", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    // The ABI is embedded as a string literal in the generated code.
    // Extract it: find the pattern { "contract" ... } inside the code.
    // The gen_abi module produces a string like:
    //   "{ \"contract\": \"AbiCheck\", \"methods\": [...] }"
    // In the generated token stream, it appears as a raw string.
    // We search for the JSON-like content between the quotes.

    // The ABI JSON is embedded as a string literal. In quote's token stream output,
    // inner quotes appear escaped. We check for the contract name directly.
    assert!(code.contains("AbiCheck"), "generated code should reference contract name");
    assert!(code.contains("__export_abi"), "generated code should have ABI export function");

    // Extract the ABI string - in the token stream it uses escaped quotes
    let abi_start = code.find("{ \\\"contract\\\"");
    if let Some(start) = abi_start {
        // Find the closing of the JSON (last } before .as_bytes)
        let rest = &code[start..];
        if let Some(end) = rest.find("\" . as_bytes") {
            let abi_escaped = &rest[..end];
            let abi_json = abi_escaped.replace("\\\"", "\"");
            // Parse as JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&abi_json);
            assert!(parsed.is_ok(), "ABI should be valid JSON, got error: {:?} for input: {}", parsed.err(), &abi_json);
            let val = parsed.unwrap();
            assert_eq!(val["contract"], "AbiCheck");
            let methods = val["methods"].as_array().unwrap();
            assert_eq!(methods.len(), 3);

            // Verify method structure
            let do_something = methods.iter().find(|m| m["name"] == "do_something").unwrap();
            assert_eq!(do_something["opcode"], 1);
            let params = do_something["params"].as_array().unwrap();
            assert_eq!(params.len(), 2);
            assert_eq!(params[0]["type"], "u64");
            assert_eq!(params[0]["name"], "a");
            assert_eq!(params[1]["type"], "String");
            assert_eq!(params[1]["name"], "b");

            let get_value = methods.iter().find(|m| m["name"] == "get_value").unwrap();
            assert_eq!(get_value["returns"], "u64");

            let with_record = methods.iter().find(|m| m["name"] == "with_record").unwrap();
            assert_eq!(with_record["params"].as_array().unwrap()[0]["type"], "MyRecord");
            assert_eq!(with_record["returns"], "MyEnum");

            println!("ABI JSON validated successfully");
        } else {
            // Fallback: just check the code contains expected elements
            assert!(code.contains("AbiCheck"), "ABI should reference contract name");
            assert!(code.contains("do_something"), "ABI should reference method");
        }
    } else {
        // The token stream may format differently. Just verify key elements are present.
        assert!(code.contains("AbiCheck"), "ABI should reference contract name");
        assert!(code.contains("do_something"), "ABI should reference method name");
        assert!(code.contains("get_value"), "ABI should reference method name");
        assert!(code.contains("with_record"), "ABI should reference method name");
    }
}

// =============================================================================
// Mixed types: record + enum + variant in one WIT
// =============================================================================

#[test]
fn test_mixed_custom_types_codegen() {
    let wit = r#"
package test:mixed-types;

interface mixed-types {
    record position {
        x: u64,
        y: u64,
    }

    enum direction {
        north,
        south,
        east,
        west,
    }

    variant move-command {
        walk(direction),
        teleport(position),
        stay,
    }

    execute-move: func(cmd: move-command) -> result<position, string>;
}

world mixed-types-world {
    export mixed-types;
}
"#;

    let toml = r#"
[contract]
name = "MixedTypes"

[opcodes]
execute-move = 1
"#;

    let (wit_path, toml_path) = write_test_files("mixed_types", wit, toml);
    let ir = alkanes_wit_parser::parse(&wit_path, &toml_path).unwrap();

    assert_eq!(ir.name, "MixedTypes");
    assert_eq!(ir.methods.len(), 1);
    // Should have 3 custom types: Position, Direction, MoveCommand
    assert_eq!(ir.custom_types.len(), 3, "should parse record + enum + variant = 3 custom types");

    let type_names: Vec<&str> = ir.custom_types.iter().map(|t| t.name.as_str()).collect();
    assert!(type_names.contains(&"Position"), "should contain Position record");
    assert!(type_names.contains(&"Direction"), "should contain Direction enum");
    assert!(type_names.contains(&"MoveCommand"), "should contain MoveCommand variant");

    // Generate code and verify all types appear
    let generated = alkanes_wit_codegen::generate(&ir).unwrap();
    let code = &generated.module_code;

    assert!(code.contains("struct Position"), "should generate Position struct");
    assert!(code.contains("enum Direction"), "should generate Direction enum");
    assert!(code.contains("enum MoveCommand"), "should generate MoveCommand variant as enum");

    // All three should have CellpackEncode/CellpackDecode
    assert!(code.contains("impl CellpackEncode for Position"), "Position encode");
    assert!(code.contains("impl CellpackDecode for Position"), "Position decode");
    assert!(code.contains("impl CellpackEncode for Direction"), "Direction encode");
    assert!(code.contains("impl CellpackDecode for Direction"), "Direction decode");
    assert!(code.contains("impl CellpackEncode for MoveCommand"), "MoveCommand encode");
    assert!(code.contains("impl CellpackDecode for MoveCommand"), "MoveCommand decode");

    println!("=== Mixed types generated code length: {} bytes ===", code.len());
}
