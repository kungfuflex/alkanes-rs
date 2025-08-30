//! Tests for the #[metashrew_core::view] procedural macro
//!
//! This module tests the procedural macro that generates view functions
//! for Metashrew indexers.

use crate::view;

/// Test function using the #[metashrew_core::view] macro
///
/// This demonstrates how the macro should be used in practice.
/// The macro will generate:
/// 1. A renamed function with __ prefix containing the original logic
/// 2. A WASM export function that handles input parsing and result export
#[view]
pub fn protorunesbyaddress(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // This is a test view function
    // In a real view function, you would query the database and return results

    // For testing, we'll just verify the input is reasonable and return test data
    if input.is_empty() {
        return Err("Input cannot be empty".into());
    }

    // Return some test data
    Ok(b"test_result_data".to_vec())
}

/// Another test view function to verify multiple functions work
#[view]
pub fn testview(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Simple test that echoes the input length
    let result = format!("input_length_{}", input.len());
    Ok(result.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_function_directly() {
        // Test the internal view function directly (with __ prefix)
        let input = b"test_input_data";

        let result = __protorunesbyaddress(input);
        assert!(result.is_ok(), "View function should succeed");
        assert_eq!(result.unwrap(), b"test_result_data".to_vec());
    }

    #[test]
    fn test_view_function_error_handling() {
        // Test error handling in view function
        let empty_input = b"";

        let result = __protorunesbyaddress(empty_input);
        assert!(
            result.is_err(),
            "View function should return error for empty input"
        );
    }

    #[test]
    fn test_multiple_view_functions() {
        // Test that multiple view functions can be defined
        let input = b"test_data";

        let result1 = __protorunesbyaddress(input);
        let result2 = __testview(input);

        assert!(result1.is_ok(), "First view function should succeed");
        assert!(result2.is_ok(), "Second view function should succeed");

        assert_eq!(result1.unwrap(), b"test_result_data".to_vec());
        assert_eq!(result2.unwrap(), b"input_length_9".to_vec());
    }

    #[test]
    fn test_macro_preserves_function_signature() {
        // Test that the macro preserves the correct function signature
        let input: &[u8] = b"signature_test";

        let result: Result<Vec<u8>, Box<dyn std::error::Error>> = __protorunesbyaddress(input);
        assert!(result.is_ok(), "Function signature should be preserved");
    }

    #[test]
    fn test_macro_compilation() {
        // This test simply verifies that the view macro compiles without errors
        // The fact that this test compiles and runs means the macro is working
        assert!(true, "View macro compilation test passed");
    }
}
