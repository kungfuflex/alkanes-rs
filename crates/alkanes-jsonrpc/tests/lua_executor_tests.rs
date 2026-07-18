use serde_json::json;

#[tokio::test]
async fn test_lua_basic_execution() {
    // This test verifies basic Lua execution works
    let lua = mlua::Lua::new();
    
    let result: mlua::Value = lua.load("return {result = 42}").eval().unwrap();
    
    if let mlua::Value::Table(table) = result {
        let value: i32 = table.get("result").unwrap();
        assert_eq!(value, 42);
    } else {
        panic!("Expected table result");
    }
}

#[tokio::test]
async fn test_lua_args_access() {
    // Test that Lua script can access args
    let lua = mlua::Lua::new();
    let globals = lua.globals();
    
    // Set up args table
    let args_table = lua.create_table().unwrap();
    args_table.set(1, "test_value").unwrap();
    globals.set("args", args_table).unwrap();
    
    let result: String = lua.load("return args[1]").eval().unwrap();
    assert_eq!(result, "test_value");
}

#[tokio::test]
async fn test_lua_script_with_rpc_mock() {
    // Test Lua script that uses _RPC
    let lua = mlua::Lua::new();
    let globals = lua.globals();
    
    // Create mock _RPC table
    let rpc_table = lua.create_table().unwrap();
    
    // Mock method that returns a fixed value
    let mock_method = lua.create_function(|lua_ctx, ()| {
        let table = lua_ctx.create_table()?;
        table.set("txid", "mock_txid_123")?;
        table.set("vout", 0)?;
        table.set("value", 1000)?;
        Ok(table)
    }).unwrap();
    
    rpc_table.set("esplora_addressutxo", mock_method).unwrap();
    globals.set("_RPC", rpc_table).unwrap();
    
    // Execute script that uses _RPC
    let script = r#"
        local utxo = _RPC.esplora_addressutxo()
        return {count = 1, first_txid = utxo.txid}
    "#;
    
    let result: mlua::Table = lua.load(script).eval().unwrap();
    let count: i32 = result.get("count").unwrap();
    let first_txid: String = result.get("first_txid").unwrap();
    assert_eq!(count, 1);
    assert_eq!(first_txid, "mock_txid_123");
}

#[test]
fn test_address_utxos_with_txs_script_syntax() {
    // Verify our embedded script has valid Lua syntax
    let script = include_str!("../../../lua/address_utxos_with_txs.lua");
    
    let lua = mlua::Lua::new();
    let result = lua.load(script).eval::<mlua::Value>();
    
    // Should fail because args and _RPC aren't defined, but syntax should be valid
    // We're just checking it parses, not that it runs
    match result {
        Err(e) => {
            let err_msg = e.to_string();
            // Should fail on undefined variable, not syntax error
            assert!(
                err_msg.contains("args") || err_msg.contains("_RPC") || err_msg.contains("attempt to"),
                "Script should fail on undefined variable, not syntax error. Got: {}",
                err_msg
            );
        }
        Ok(_) => {
            // Script might return without error if it doesn't try to access undefined vars immediately
        }
    }
}

#[test]
fn test_all_embedded_lua_scripts_valid_syntax() {
    let scripts = vec![
        ("address_utxos_with_txs", include_str!("../../../lua/address_utxos_with_txs.lua")),
        ("balances", include_str!("../../../lua/balances.lua")),
        ("batch_utxo_balances", include_str!("../../../lua/batch_utxo_balances.lua")),
        ("multicall", include_str!("../../../lua/multicall.lua")),
    ];
    
    for (name, script) in scripts {
        let lua = mlua::Lua::new();
        let result = lua.load(script).eval::<mlua::Value>();
        
        // Scripts should parse without syntax errors
        // They may fail on execution due to missing context, but not on parsing
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    !err_msg.contains("syntax error") && !err_msg.contains("unexpected symbol"),
                    "Script '{}' has syntax error: {}",
                    name,
                    err_msg
                );
            }
            Ok(_) => {
                // Script parsed successfully
            }
        }
    }
}

#[tokio::test]
async fn test_esplora_address_utxo_rpc_call_format() {
    // Test that we can format the esplora_address::utxo call correctly
    let _method = "esplora_address::utxo";
    let address = "bcrt1p705x8h5dy67x7tgdu6wv2crq333sdx6h776vc929rxcdlxs5wj2s7h296c";
    let params = json!([address]);
    
    // Verify JSON formatting
    assert_eq!(params[0].as_str().unwrap(), address);
}

#[test]
fn test_lua_script_sha256_hashing() {
    use sha2::{Digest, Sha256};
    
    let script = "return {result = 42}";
    let mut hasher = Sha256::new();
    hasher.update(script.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    
    // Hash should be 64 hex characters
    assert_eq!(hash.len(), 64);
    
    // Same script should produce same hash
    let mut hasher2 = Sha256::new();
    hasher2.update(script.as_bytes());
    let hash2 = format!("{:x}", hasher2.finalize());
    
    assert_eq!(hash, hash2);
}
