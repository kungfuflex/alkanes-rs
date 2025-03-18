use crate::tests::helpers::clear;
use crate::tests::std::alkanes_std_auth_token_build;
use crate::tests::std::alkanes_std_genesis_alkane_build;
use crate::tests::std::alkanes_std_genesis_protorune_build;
use crate::tests::std::alkanes_std_merkle_distributor_build;
use crate::tests::std::alkanes_std_orbital_build;
use crate::tests::std::alkanes_std_owned_token_build;
use crate::tests::std::alkanes_std_proxy_build;
use crate::tests::std::alkanes_std_test_build;
use crate::tests::std::alkanes_std_upgradeable_build;
use crate::vm::fuel::FuelTank;
use crate::vm::instance::AlkanesInstance;
use crate::vm::runtime::AlkanesRuntimeContext;
use alkanes::vm::fuel::VirtualFuelBytes;
use anyhow::Result;
#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::create_block_with_coinbase_tx;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use wasm_bindgen_test::wasm_bindgen_test;

// Helper function to test the ABI of a contract
fn test_contract_abi(
    contract_name: &str,
    contract_bytes: Vec<u8>,
    expected_methods: Vec<(&str, u128, Vec<(&str, &str)>, &str)>,
) -> Result<()> {
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));

    // Create a new instance of the contract
    let mut instance = AlkanesInstance::from_alkane(context, Arc::new(contract_bytes), 100000000)?;

    // Call the __meta function to get the ABI
    let abi_bytes = instance.call_meta()?;

    // Convert the ABI bytes to a string and parse as JSON
    let abi_string = String::from_utf8(abi_bytes.clone())?;
    let abi_json: Value = serde_json::from_slice(&abi_bytes)?;

    // Print the ABI for debugging
    println!("{} ABI: {}", contract_name, abi_string);

    // Verify the contract name
    assert_eq!(abi_json["contract"], contract_name);

    // Verify that methods array exists
    assert!(abi_json["methods"].is_array());
    let methods = abi_json["methods"].as_array().unwrap();

    // Verify that we have the expected number of methods
    assert_eq!(
        methods.len(),
        expected_methods.len(),
        "Unexpected number of methods in ABI for {}",
        contract_name
    );

    // Verify each method
    for (expected_name, expected_opcode, expected_params, expected_return_type) in expected_methods
    {
        // Find the method in the ABI
        let method = methods
            .iter()
            .find(|m| m["name"] == expected_name)
            .unwrap_or_else(|| {
                panic!(
                    "Method {} not found in ABI for {}",
                    expected_name, contract_name
                )
            });

        // Verify the opcode
        assert_eq!(
            method["opcode"].as_u64().unwrap() as u128,
            expected_opcode,
            "Incorrect opcode for method {} in {}",
            expected_name,
            contract_name
        );

        // Verify the parameters
        let params = method["params"].as_array().unwrap();
        assert_eq!(
            params.len(),
            expected_params.len(),
            "Incorrect number of parameters for method {} in {}",
            expected_name,
            contract_name
        );

        // Verify each parameter type and name
        for (i, (expected_param_name, expected_param_type)) in expected_params.iter().enumerate() {
            assert_eq!(
                params[i]["type"].as_str().unwrap(),
                *expected_param_type,
                "Incorrect parameter type at index {} for method {} in {}",
                i,
                expected_name,
                contract_name
            );

            assert_eq!(
                params[i]["name"].as_str().unwrap(),
                *expected_param_name,
                "Incorrect parameter name at index {} for method {} in {}",
                i,
                expected_name,
                contract_name
            );
        }

        // Verify the return type
        if method.get("returns").is_some() {
            assert_eq!(
                method["returns"].as_str().unwrap(),
                expected_return_type,
                "Incorrect return type for method {} in {}",
                expected_name,
                contract_name
            );
        } else {
            assert_eq!(
                expected_return_type, "void",
                "Expected void return type for method {} in {}, but no return type was specified",
                expected_name, contract_name
            );
        }
    }

    Ok(())
}

#[wasm_bindgen_test]
fn test_owned_token_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        (
            "initialize",
            0,
            vec![("auth_token_units", "u128"), ("token_units", "u128")],
            "void",
        ),
        ("mint", 77, vec![("token_units", "u128")], "void"),
        (
            "set_name_and_symbol",
            88,
            vec![("name", "String"), ("symbol", "String")],
            "void",
        ),
        ("get_name", 99, vec![], "String"),
        ("get_symbol", 100, vec![], "String"),
        ("get_total_supply", 101, vec![], "u128"),
        ("get_data", 1000, vec![], "Vec<u8>"),
    ];

    test_contract_abi(
        "OwnedToken",
        alkanes_std_owned_token_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_auth_token_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("initialize", 0, vec![("amount", "u128")], "void"),
        ("authenticate", 1, vec![], "void"),
        ("get_name", 99, vec![], "String"),
        ("get_symbol", 100, vec![], "String"),
    ];

    test_contract_abi(
        "AuthToken",
        alkanes_std_auth_token_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_proxy_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("initialize", 0, vec![], "void"),
        ("call_witness", 1, vec![("witness_index", "u128")], "void"),
        (
            "delegatecall_witness",
            2,
            vec![("witness_index", "u128")],
            "void",
        ),
        ("call_inputs", 3, vec![], "void"),
        ("delegatecall_inputs", 4, vec![], "void"),
    ];

    test_contract_abi(
        "Proxy",
        alkanes_std_proxy_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_upgradeable_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        (
            "initialize",
            0x7fff,
            vec![
                ("block", "u128"),
                ("tx", "u128"),
                ("auth_token_units", "u128"),
            ],
            "void",
        ),
        (
            "upgrade",
            0x7ffe,
            vec![("block", "u128"), ("tx", "u128")],
            "void",
        ),
        ("delegate", 0x7ffd, vec![], "void"),
    ];

    test_contract_abi(
        "Upgradeable",
        alkanes_std_upgradeable_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_logger_alkane_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("self_call", 2, vec![], "void"),
        ("check_incoming", 3, vec![], "void"),
        ("mint_tokens", 4, vec![], "void"),
        ("return_data_1", 5, vec![], "Vec<u8>"),
        ("get_transaction", 50, vec![], "void"),
        ("hash_loop", 78, vec![], "void"),
        ("return_default_data", 99, vec![], "Vec<u8>"),
    ];

    test_contract_abi(
        "LoggerAlkane",
        alkanes_std_test_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_orbital_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("initialize", 0, vec![], "void"),
        ("get_name", 99, vec![], "String"),
        ("get_symbol", 100, vec![], "String"),
        ("get_total_supply", 101, vec![], "u128"),
        ("get_data", 1000, vec![], "Vec<u8>"),
    ];

    test_contract_abi(
        "Orbital",
        alkanes_std_orbital_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_merkle_distributor_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        (
            "initialize",
            0,
            vec![("length", "u128"), ("root_bytes", "u128")],
            "void",
        ),
        ("claim", 1, vec![], "void"),
    ];

    test_contract_abi(
        "MerkleDistributor",
        alkanes_std_merkle_distributor_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_genesis_alkane_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("initialize", 0, vec![], "void"),
        ("mint", 77, vec![], "void"),
        ("get_name", 99, vec![], "String"),
        ("get_symbol", 100, vec![], "String"),
        ("get_total_supply", 101, vec![], "u128"),
    ];

    test_contract_abi(
        "GenesisAlkane",
        alkanes_std_genesis_alkane_build::get_bytes(),
        expected_methods,
    )
}

#[wasm_bindgen_test]
fn test_genesis_protorune_abi() -> Result<()> {
    clear();

    // Expected methods with their opcodes, parameter names and types, and return types
    let expected_methods = vec![
        ("initialize", 0, vec![], "void"),
        ("mint", 77, vec![], "void"),
        ("get_name", 99, vec![], "String"),
        ("get_symbol", 100, vec![], "String"),
        ("get_total_supply", 101, vec![], "u128"),
    ];

    test_contract_abi(
        "GenesisProtorune",
        alkanes_std_genesis_protorune_build::get_bytes(),
        expected_methods,
    )
}
