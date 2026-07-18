// Ethereum calldata encoding utilities for BRC20-prog
// Uses alloy-dyn-abi for proper ABI encoding including dynamic types

use crate::{AlkanesError, Result};
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};
#[cfg(feature = "std")]
use std::{string::{String, ToString}, vec::Vec, format};

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{hex, Address, U256};

/// Encode a function call with signature and arguments
///
/// # Arguments
/// * `signature` - Function signature (e.g., "transfer(address,uint256)" or "upgradeAndCall(address,address,bytes)")
/// * `args` - Comma-separated argument values (e.g., "0x1234...,1000" or "0x1234...,0x5678...,0x")
///
/// # Returns
/// * Hex-encoded calldata with 0x prefix
pub fn encode_function_call(signature: &str, args: &str) -> Result<String> {
    // Parse the function signature to extract parameter types
    let (_func_name, param_types) = parse_signature(signature)?;

    // Calculate function selector
    let selector = calculate_function_selector(signature)?;

    // Parse argument values
    let arg_values = parse_arguments(args, &param_types)?;

    // Encode the arguments using alloy-dyn-abi
    let encoded_args = encode_args(&param_types, &arg_values)?;

    // Concatenate selector + encoded args
    Ok(format!("0x{}{}", selector, hex::encode(&encoded_args)))
}

/// Parse function signature into name and parameter types
fn parse_signature(signature: &str) -> Result<(String, Vec<String>)> {
    // Find opening paren
    let open_paren = signature.find('(')
        .ok_or_else(|| AlkanesError::Other(format!("Invalid signature: missing '(' in {}", signature)))?;
    let close_paren = signature.rfind(')')
        .ok_or_else(|| AlkanesError::Other(format!("Invalid signature: missing ')' in {}", signature)))?;

    let func_name = signature[..open_paren].to_string();
    let params_str = &signature[open_paren + 1..close_paren];

    let param_types: Vec<String> = if params_str.is_empty() {
        Vec::new()
    } else {
        params_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    Ok((func_name, param_types))
}

/// Calculate the function selector (first 4 bytes of keccak256 hash)
fn calculate_function_selector(signature: &str) -> Result<String> {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(signature.as_bytes());
    let hash = hasher.finalize();

    // Take first 4 bytes
    let selector = &hash[0..4];
    Ok(hex::encode(selector))
}

/// Parse comma-separated arguments into values based on their types
fn parse_arguments(args: &str, param_types: &[String]) -> Result<Vec<String>> {
    if args.trim().is_empty() && param_types.is_empty() {
        return Ok(Vec::new());
    }

    if args.trim().is_empty() {
        return Err(AlkanesError::Other(format!(
            "Expected {} arguments but got none",
            param_types.len()
        )));
    }

    // Smart split that handles nested parentheses and arrays
    let arg_values = smart_split_args(args)?;

    if arg_values.len() != param_types.len() {
        return Err(AlkanesError::Other(format!(
            "Expected {} arguments but got {}",
            param_types.len(),
            arg_values.len()
        )));
    }

    Ok(arg_values)
}

/// Split arguments by comma, but respect nested structures like arrays and tuples
fn smart_split_args(args: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in args.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                result.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        result.push(current.trim().to_string());
    }

    Ok(result)
}

/// Parse a string value into a DynSolValue based on the type
fn parse_value(type_str: &str, value: &str) -> Result<DynSolValue> {
    let sol_type: DynSolType = type_str.parse()
        .map_err(|e| AlkanesError::Other(format!("Invalid type '{}': {:?}", type_str, e)))?;

    match &sol_type {
        DynSolType::Address => {
            let addr: Address = value.parse()
                .map_err(|e| AlkanesError::Other(format!("Invalid address '{}': {:?}", value, e)))?;
            Ok(DynSolValue::Address(addr))
        }
        DynSolType::Bytes => {
            let bytes = parse_bytes(value)?;
            Ok(DynSolValue::Bytes(bytes.to_vec()))
        }
        DynSolType::FixedBytes(size) => {
            let bytes = parse_bytes(value)?;
            // Pad or truncate to the fixed size
            let mut fixed = vec![0u8; *size];
            let copy_len = bytes.len().min(*size);
            fixed[..copy_len].copy_from_slice(&bytes[..copy_len]);
            Ok(DynSolValue::FixedBytes(alloy_primitives::FixedBytes::from_slice(&fixed), *size))
        }
        DynSolType::String => {
            // Remove quotes if present
            let s = value.trim_matches('"').trim_matches('\'');
            Ok(DynSolValue::String(s.to_string()))
        }
        DynSolType::Bool => {
            let b = match value.to_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => return Err(AlkanesError::Other(format!("Invalid bool value: {}", value))),
            };
            Ok(DynSolValue::Bool(b))
        }
        DynSolType::Int(bits) => {
            let n: i128 = value.parse()
                .map_err(|e| AlkanesError::Other(format!("Invalid int value '{}': {:?}", value, e)))?;
            Ok(DynSolValue::Int(alloy_primitives::I256::try_from(n)
                .map_err(|e| AlkanesError::Other(format!("Int overflow: {:?}", e)))?, *bits))
        }
        DynSolType::Uint(bits) => {
            // Handle hex or decimal
            let n: U256 = if value.starts_with("0x") {
                U256::from_str_radix(&value[2..], 16)
                    .map_err(|e| AlkanesError::Other(format!("Invalid hex uint '{}': {:?}", value, e)))?
            } else {
                U256::from_str_radix(value, 10)
                    .map_err(|e| AlkanesError::Other(format!("Invalid uint '{}': {:?}", value, e)))?
            };
            Ok(DynSolValue::Uint(n, *bits))
        }
        DynSolType::Array(inner) => {
            // Parse array like [val1,val2,val3]
            let inner_type = inner.as_ref();
            let values = parse_array_values(value)?;
            let mut items = Vec::new();
            for v in values {
                items.push(parse_value(&inner_type.to_string(), &v)?);
            }
            Ok(DynSolValue::Array(items))
        }
        DynSolType::FixedArray(inner, size) => {
            let inner_type = inner.as_ref();
            let values = parse_array_values(value)?;
            if values.len() != *size {
                return Err(AlkanesError::Other(format!(
                    "Fixed array expects {} elements, got {}",
                    size,
                    values.len()
                )));
            }
            let mut items = Vec::new();
            for v in values {
                items.push(parse_value(&inner_type.to_string(), &v)?);
            }
            Ok(DynSolValue::FixedArray(items))
        }
        DynSolType::Tuple(types) => {
            // Parse tuple like (val1,val2,val3)
            let values = parse_tuple_values(value)?;
            if values.len() != types.len() {
                return Err(AlkanesError::Other(format!(
                    "Tuple expects {} elements, got {}",
                    types.len(),
                    values.len()
                )));
            }
            let mut items = Vec::new();
            for (t, v) in types.iter().zip(values.iter()) {
                items.push(parse_value(&t.to_string(), v)?);
            }
            Ok(DynSolValue::Tuple(items))
        }
        _ => Err(AlkanesError::Other(format!("Unsupported type: {}", type_str))),
    }
}

/// Parse bytes from hex string (with or without 0x prefix)
fn parse_bytes(value: &str) -> Result<Vec<u8>> {
    let hex_str = value.strip_prefix("0x").unwrap_or(value);
    if hex_str.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(hex_str)
        .map_err(|e| AlkanesError::Other(format!("Invalid hex bytes '{}': {:?}", value, e)))
}

/// Parse array values from string like [val1,val2,val3]
fn parse_array_values(value: &str) -> Result<Vec<String>> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(AlkanesError::Other(format!("Invalid array format: {}", value)));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    smart_split_args(inner)
}

/// Parse tuple values from string like (val1,val2,val3)
fn parse_tuple_values(value: &str) -> Result<Vec<String>> {
    let trimmed = value.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return Err(AlkanesError::Other(format!("Invalid tuple format: {}", value)));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    smart_split_args(inner)
}

/// Encode arguments using alloy-dyn-abi
fn encode_args(param_types: &[String], arg_values: &[String]) -> Result<Vec<u8>> {
    if param_types.is_empty() {
        return Ok(Vec::new());
    }

    // Parse each value according to its type
    let mut values = Vec::new();
    for (type_str, value) in param_types.iter().zip(arg_values.iter()) {
        let parsed = parse_value(type_str, value)?;
        values.push(parsed);
    }

    // Create a tuple of all values and encode
    let tuple = DynSolValue::Tuple(values);
    let encoded = tuple.abi_encode_params();

    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_selector() {
        // transfer(address,uint256) should have selector 0xa9059cbb
        let selector = calculate_function_selector("transfer(address,uint256)").unwrap();
        assert_eq!(selector, "a9059cbb");
    }

    #[test]
    fn test_upgrade_and_call_selector() {
        // upgradeAndCall(address,address,bytes) should have selector 0x9623609d
        let selector = calculate_function_selector("upgradeAndCall(address,address,bytes)").unwrap();
        assert_eq!(selector, "9623609d");
    }

    #[test]
    fn test_encode_transfer() {
        let calldata = encode_function_call(
            "transfer(address,uint256)",
            "0x1234567890123456789012345678901234567890,1000"
        ).unwrap();

        assert!(calldata.starts_with("0xa9059cbb")); // function selector
        // Address should be padded to 32 bytes
        assert!(calldata.contains("0000000000000000000000001234567890123456789012345678901234567890"));
    }

    #[test]
    fn test_encode_upgrade_and_call_empty_bytes() {
        // This is the exact case from the user's issue
        let calldata = encode_function_call(
            "upgradeAndCall(address,address,bytes)",
            "0xdBB5b6A1D422fca2813cF486e5f986ADB09D8337,0x7d42fce0c3f8d80a5297aa8d6ccaada4ec591e65,0x"
        ).unwrap();

        // Expected from ethers.js:
        // 0x9623609d
        // 000000000000000000000000dbb5b6a1d422fca2813cf486e5f986adb09d8337  // address 1
        // 0000000000000000000000007d42fce0c3f8d80a5297aa8d6ccaada4ec591e65  // address 2
        // 0000000000000000000000000000000000000000000000000000000000000060  // offset to bytes (96 = 0x60)
        // 0000000000000000000000000000000000000000000000000000000000000000  // length of bytes (0)

        let expected = "0x9623609d\
            000000000000000000000000dbb5b6a1d422fca2813cf486e5f986adb09d8337\
            0000000000000000000000007d42fce0c3f8d80a5297aa8d6ccaada4ec591e65\
            0000000000000000000000000000000000000000000000000000000000000060\
            0000000000000000000000000000000000000000000000000000000000000000";

        assert_eq!(calldata.to_lowercase(), expected.to_lowercase());
    }

    #[test]
    fn test_encode_bytes_with_data() {
        // Test bytes with actual data
        let calldata = encode_function_call(
            "execute(bytes)",
            "0xdeadbeef"
        ).unwrap();

        // Should have:
        // - 4 byte selector
        // - 32 bytes offset (0x20 = 32)
        // - 32 bytes length (4)
        // - 32 bytes data (deadbeef padded)
        assert!(calldata.len() > 10);
    }

    #[test]
    fn test_parse_signature() {
        let (name, params) = parse_signature("transfer(address,uint256)").unwrap();
        assert_eq!(name, "transfer");
        assert_eq!(params, vec!["address", "uint256"]);

        let (name2, params2) = parse_signature("noArgs()").unwrap();
        assert_eq!(name2, "noArgs");
        assert!(params2.is_empty());
    }

    #[test]
    fn test_encode_bool() {
        let calldata = encode_function_call("setFlag(bool)", "true").unwrap();
        assert!(calldata.ends_with("0000000000000000000000000000000000000000000000000000000000000001"));

        let calldata2 = encode_function_call("setFlag(bool)", "false").unwrap();
        assert!(calldata2.ends_with("0000000000000000000000000000000000000000000000000000000000000000"));
    }
}
