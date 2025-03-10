use crate::tests::helpers::clear;
use crate::tests::std::alkanes_std_owned_token_build;
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

#[wasm_bindgen_test]
fn test_owned_token_abi() -> Result<()> {
    clear();
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));
    // Create a new instance of the OwnedToken contract
    let mut instance = AlkanesInstance::from_alkane(
        context,
        Arc::new(alkanes_std_owned_token_build::get_bytes()),
        100000000,
    )?;

    // Call the __meta function to get the ABI
    let abi_bytes = instance.call_meta()?;

    // Convert the ABI bytes to a string and parse as JSON
    let abi_string = String::from_utf8(abi_bytes.clone())?;
    let abi_json: Value = serde_json::from_slice(&abi_bytes)?;

    // Print the ABI for debugging
    println!("OwnedToken ABI: {}", abi_string);

    // Verify the contract name
    assert_eq!(abi_json["contract"], "OwnedToken");

    // Verify that methods array exists
    assert!(abi_json["methods"].is_array());
    let methods = abi_json["methods"].as_array().unwrap();

    // Expected methods with their opcodes and parameter counts
    let expected_methods = vec![
        ("initialize", 0, vec!["u128", "u128"]),
        ("mint", 77, vec!["u128"]),
        ("get_name", 99, vec![]),
        ("get_symbol", 100, vec![]),
        ("get_total_supply", 101, vec![]),
        ("get_data", 1000, vec![]),
    ];

    // Verify that we have the expected number of methods
    assert_eq!(
        methods.len(),
        expected_methods.len(),
        "Unexpected number of methods in ABI"
    );

    // Verify each method
    for (expected_name, expected_opcode, expected_params) in expected_methods {
        // Find the method in the ABI
        let method = methods
            .iter()
            .find(|m| m["name"] == expected_name)
            .unwrap_or_else(|| panic!("Method {} not found in ABI", expected_name));

        // Verify the opcode
        assert_eq!(
            method["opcode"].as_u64().unwrap() as u128,
            expected_opcode,
            "Incorrect opcode for method {}",
            expected_name
        );

        // Verify the parameters
        let params = method["params"].as_array().unwrap();
        assert_eq!(
            params.len(),
            expected_params.len(),
            "Incorrect number of parameters for method {}",
            expected_name
        );

        // Verify each parameter type
        for (i, expected_param) in expected_params.iter().enumerate() {
            assert_eq!(
                params[i].as_str().unwrap(),
                *expected_param,
                "Incorrect parameter type at index {} for method {}",
                i,
                expected_name
            );
        }
    }

    Ok(())
}
