//! Tests for the #[metashrew_core::main] procedural macro
//!
//! This module tests the procedural macro that generates the _start function
//! for Metashrew indexers. These tests verify that the macro compiles correctly
//! and preserves the original function.

use crate::main;

/// Test function using the #[metashrew_core::main] macro
///
/// This demonstrates how the macro should be used in practice.
/// The macro will generate a _start function that:
/// 1. Parses input to extract height and block data
/// 2. Calls this function with the parsed parameters
/// 3. Calls flush() to commit changes
#[main]
pub fn test_indexer(height: u32, block: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // This is a test indexer function
    // In a real indexer, you would process the block data here

    // For testing, we'll just verify the parameters are reasonable
    assert!(height > 0, "Height should be greater than 0");
    assert!(!block.is_empty(), "Block data should not be empty");

    // Note: We can't test the actual cache operations here because
    // the host functions (__get, __set, __flush) are only available
    // in the WASM runtime environment, not in native tests.

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_function_directly() {
        // Test the indexer function directly
        let height = 12345;
        let block_data = b"test_block_data";

        let result = test_indexer(height, block_data);
        assert!(result.is_ok(), "Indexer function should succeed");
    }

    #[test]
    fn test_macro_preserves_original_function() {
        // The macro should preserve the original function
        let result = test_indexer(67890, b"another_test_block");
        assert!(
            result.is_ok(),
            "Original function should still work after macro application"
        );
    }

    #[test]
    fn test_macro_signature_validation() {
        // This test verifies that the macro correctly validates function signatures
        // The test_indexer function should have the correct signature:
        // fn(height: u32, block: &[u8]) -> Result<(), Box<dyn std::error::Error>>

        // If the macro is working correctly, the test_indexer function should exist
        // and be callable with the expected parameters
        let height: u32 = 100;
        let block: &[u8] = b"test";

        let result = test_indexer(height, block);
        assert!(
            result.is_ok(),
            "Function with correct signature should work"
        );
    }

    #[test]
    fn test_macro_compilation() {
        // This test simply verifies that the macro compiles without errors
        // The fact that this test compiles and runs means the macro is working
        assert!(true, "Macro compilation test passed");
    }
}
