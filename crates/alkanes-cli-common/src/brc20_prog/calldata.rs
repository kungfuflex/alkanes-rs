// Ethereum calldata encoding utilities for BRC20-prog

use crate::{AlkanesError, Result};
#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};
#[cfg(feature = "std")]
use std::{string::{String, ToString}, vec::Vec, format};
use sha3::{Digest, Keccak256};

/// Encode a function call with signature and arguments
/// 
/// # Arguments
/// * `signature` - Function signature (e.g., "transfer(address,uint256)")
/// * `args` - Comma-separated argument values (e.g., "0x1234...,1000")
/// 
/// # Returns
/// * Hex-encoded calldata with 0x prefix
pub fn encode_function_call(signature: &str, args: &str) -> Result<String> {
    // Calculate function selector (first 4 bytes of keccak256 hash)
    let selector = calculate_function_selector(signature)?;
    
    // Parse and encode arguments
    let encoded_args = encode_arguments(args)?;
    
    // Concatenate selector + encoded args
    Ok(format!("0x{}{}", selector, encoded_args))
}

/// Calculate the function selector (first 4 bytes of keccak256 hash)
fn calculate_function_selector(signature: &str) -> Result<String> {
    let mut hasher = Keccak256::new();
    hasher.update(signature.as_bytes());
    let hash = hasher.finalize();
    
    // Take first 4 bytes
    let selector = &hash[0..4];
    Ok(hex::encode(selector))
}

/// Encode arguments according to Ethereum ABI encoding
/// This is a simplified version that handles common types
fn encode_arguments(args: &str) -> Result<String> {
    if args.trim().is_empty() {
        return Ok(String::new());
    }
    
    let mut encoded = String::new();
    let arg_list: Vec<&str> = args.split(',').map(|s| s.trim()).collect();
    
    for arg in arg_list {
        let encoded_arg = encode_single_argument(arg)?;
        encoded.push_str(&encoded_arg);
    }
    
    Ok(encoded)
}

/// Encode a single argument
/// Supports: addresses, uint256, bool
fn encode_single_argument(arg: &str) -> Result<String> {
    // Handle address (0x prefixed hex with 20 bytes)
    if arg.starts_with("0x") && arg.len() == 42 {
        // Address: pad to 32 bytes (64 hex chars)
        let addr = &arg[2..]; // Remove 0x prefix
        return Ok(format!("{:0>64}", addr));
    }
    
    // Handle hex data (0x prefixed but not an address)
    if arg.starts_with("0x") {
        let hex_data = &arg[2..];
        // Pad to 32 bytes
        return Ok(format!("{:0>64}", hex_data));
    }
    
    // Handle boolean
    if arg == "true" || arg == "false" {
        let value = if arg == "true" { "1" } else { "0" };
        return Ok(format!("{:0>64}", value));
    }
    
    // Handle uint256 (decimal number)
    if let Ok(num) = arg.parse::<u128>() {
        return Ok(format!("{:064x}", num));
    }
    
    Err(AlkanesError::Other(format!(
        "Unable to encode argument: {}. Supported types: address (0x...), uint256 (number), bool (true/false), hex (0x...)",
        arg
    )))
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
    fn test_encode_address() {
        let encoded = encode_single_argument("0x1234567890123456789012345678901234567890").unwrap();
        assert_eq!(encoded.len(), 64);
        assert!(encoded.ends_with("1234567890123456789012345678901234567890"));
    }

    #[test]
    fn test_encode_uint256() {
        let encoded = encode_single_argument("1000").unwrap();
        assert_eq!(encoded, format!("{:064x}", 1000));
    }

    #[test]
    fn test_encode_bool() {
        let encoded_true = encode_single_argument("true").unwrap();
        let encoded_false = encode_single_argument("false").unwrap();
        
        assert_eq!(encoded_true, format!("{:0>64}", "1"));
        assert_eq!(encoded_false, format!("{:0>64}", "0"));
    }

    #[test]
    fn test_encode_function_call() {
        let calldata = encode_function_call(
            "transfer(address,uint256)",
            "0x1234567890123456789012345678901234567890,1000"
        ).unwrap();
        
        assert!(calldata.starts_with("0xa9059cbb")); // function selector
        assert!(calldata.len() > 10); // selector + args
    }
}
