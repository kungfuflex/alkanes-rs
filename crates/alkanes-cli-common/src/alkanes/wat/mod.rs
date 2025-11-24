//! WebAssembly Text (WAT) templates for transaction scripts
//!
//! This module contains WAT templates that can be compiled to WASM and executed
//! using the `--envelope` flag with simulate calls. These scripts run on-chain
//! to perform complex operations like path optimization in a single call.

/// Include WAT template as a static string
pub const OPTIMIZE_SWAP_PATH_WAT: &str = include_str!("optimize_swap_path.wat");

/// Compile WAT to WASM bytes
/// 
/// This uses the `wat` crate to parse and compile WAT to WASM.
/// The `wat` crate is no_std compatible and works in WASM environments.
#[cfg(feature = "std")]
pub fn compile_wat_to_wasm(wat_source: &str) -> Result<Vec<u8>, String> {
    wat::parse_str(wat_source).map_err(|e| format!("WAT compilation error: {}", e))
}

/// Compile WAT to WASM bytes (no_std version)
#[cfg(not(feature = "std"))]
pub fn compile_wat_to_wasm(wat_source: &str) -> Result<Vec<u8>, String> {
    wat::parse_str(wat_source).map_err(|e| format!("WAT compilation error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_optimize_swap_path() {
        let result = compile_wat_to_wasm(OPTIMIZE_SWAP_PATH_WAT);
        assert!(result.is_ok(), "WAT compilation should succeed");
        
        let wasm_bytes = result.unwrap();
        assert!(wasm_bytes.len() > 0, "WASM output should not be empty");
        
        // Verify WASM magic number (0x00 0x61 0x73 0x6D)
        assert_eq!(&wasm_bytes[0..4], b"\0asm", "WASM magic number should be present");
    }
}
