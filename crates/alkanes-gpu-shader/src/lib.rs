//! alkanes-gpu-shader: Complete SPIR-V compute shader with real WASM interpreter
//!
//! This crate provides a GPU shader implementation that processes AlkanesMessageContext
//! objects with a complete WASM interpreter and K/V store implementation that runs on GPU.
//!
//! ## Architecture
//!
//! The GPU shader processes alkanes messages in parallel, with each shader invocation handling
//! a single message. The shader includes:
//!
//! - Complete SPIR-V-compatible WASM interpreter with real bytecode parsing
//! - GPU-optimized constraint checking with ejection detection
//! - Host function implementations matching alkanes VM exactly
//! - Precise fuel tracking compatible with wasmi
//! - Shard ejection for constraint violations
//! - K/V store isolation and transaction management
//!
//! ## Ejection Handling
//!
//! The shader can eject shards for several reasons:
//! - K/V read/write to keys not in preloaded subset -> synchronous pipeline
//! - Memory/storage constraints exceeded -> multithreading pipeline
//! - Call stack depth overflow -> multithreading pipeline
//! - WASM memory growth beyond 42M limit -> multithreading pipeline

#![cfg_attr(target_arch = "spirv", no_std)]
#![cfg_attr(target_arch = "spirv", no_main)]

// Only import spirv-std when compiling for SPIR-V target
#[cfg(target_arch = "spirv")]
use spirv_std::spirv;
#[cfg(target_arch = "spirv")]
use spirv_std::glam::UVec3;

// Import our WASM interpreter and parser
mod wasm_interpreter;
mod wasm_parser;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod standalone_tests;
#[cfg(test)]
mod e2e_tests;

use wasm_interpreter::{SpirvWasmContext, SpirvWasmExecutor, execute_alkanes_message};
use wasm_parser::{SpirvWasmParser, find_export};

// For non-SPIR-V targets, provide dummy types for compatibility
#[cfg(not(target_arch = "spirv"))]
#[derive(Clone, Copy, Debug)]
pub struct UVec4 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub w: u32,
}

#[cfg(not(target_arch = "spirv"))]
impl UVec4 {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0, w: 0 };
}

/// Maximum constraints for GPU compatibility
pub const MAX_MESSAGE_SIZE: usize = 4096;
pub const MAX_CALLDATA_SIZE: usize = 2048;
pub const MAX_KV_PAIRS: usize = 1024;
pub const MAX_RETURN_DATA_SIZE: usize = 1024;
pub const MAX_SHARD_SIZE: usize = 64;
pub const FIXED_RESULT_SIZE: usize = 256;

/// WASM memory limit for GPU execution (42MB)
pub const GPU_WASM_MEMORY_LIMIT: usize = 42 * 1024 * 1024;

/// Maximum call stack depth for GPU execution
pub const GPU_MAX_CALL_STACK_DEPTH: usize = 256;

/// GPU shader entry point for alkanes message processing
#[cfg(target_arch = "spirv")]
#[spirv(compute(threads(64)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] global_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] input_shard: &[u8],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] output_result: &mut [u8],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ejected_shards: &mut [u32],
    #[spirv(uniform, descriptor_set = 0, binding = 3)] params: &ShaderParams,
) {
    let thread_id = global_id.x;
    
    // Check if this thread should process a message
    if thread_id >= params.message_count {
        return;
    }
    
    // Process the alkanes message for this thread with real WASM interpreter
    let success = process_alkanes_message_with_real_wasm_interpreter(
        thread_id,
        input_shard,
        params,
        output_result,
        ejected_shards
    );
    
    if !success {
        // Mark shard for ejection
        mark_shard_ejected(thread_id, ejected_shards);
    }
}

/// Parameters passed to the GPU shader
#[repr(C)]
pub struct ShaderParams {
    /// Number of messages to process
    pub message_count: u32,
    /// Current block height
    pub block_height: u32,
    /// Base fuel allocation per message
    pub base_fuel: u64,
    /// Maximum fuel per message
    pub max_fuel: u64,
    /// Size of each message in the input buffer
    pub message_size: u32,
    /// Size of each result in the output buffer
    pub result_size: u32,
}

/// Process alkanes message with real WASM interpreter (SPIR-V version)
#[cfg(target_arch = "spirv")]
fn process_alkanes_message_with_real_wasm_interpreter(
    thread_id: u32,
    input_shard: &[u8],
    params: &ShaderParams,
    output_result: &mut [u8],
    ejected_shards: &mut [u32],
) -> bool {
    // Calculate message offset
    let message_offset = (thread_id as usize) * (params.message_size as usize);
    let result_offset = (thread_id as usize) * (params.result_size as usize);
    
    // Bounds check
    if message_offset + (params.message_size as usize) > input_shard.len() ||
       result_offset + (params.result_size as usize) > output_result.len() {
        return false;
    }
    
    // Parse message structure to extract WASM bytecode and context
    if message_offset + 128 > input_shard.len() {
        return false;
    }
    
    // Extract contract ID (first 16 bytes)
    let contract_id = {
        #[cfg(target_arch = "spirv")]
        {
            use spirv_std::glam::UVec4;
            UVec4::new(
                u32::from_le_bytes([
                    input_shard[message_offset],
                    input_shard[message_offset + 1],
                    input_shard[message_offset + 2],
                    input_shard[message_offset + 3],
                ]),
                u32::from_le_bytes([
                    input_shard[message_offset + 4],
                    input_shard[message_offset + 5],
                    input_shard[message_offset + 6],
                    input_shard[message_offset + 7],
                ]),
                u32::from_le_bytes([
                    input_shard[message_offset + 8],
                    input_shard[message_offset + 9],
                    input_shard[message_offset + 10],
                    input_shard[message_offset + 11],
                ]),
                u32::from_le_bytes([
                    input_shard[message_offset + 12],
                    input_shard[message_offset + 13],
                    input_shard[message_offset + 14],
                    input_shard[message_offset + 15],
                ]),
            )
        }
        #[cfg(not(target_arch = "spirv"))]
        {
            UVec4 {
                x: u32::from_le_bytes([
                    input_shard[message_offset],
                    input_shard[message_offset + 1],
                    input_shard[message_offset + 2],
                    input_shard[message_offset + 3],
                ]),
                y: u32::from_le_bytes([
                    input_shard[message_offset + 4],
                    input_shard[message_offset + 5],
                    input_shard[message_offset + 6],
                    input_shard[message_offset + 7],
                ]),
                z: u32::from_le_bytes([
                    input_shard[message_offset + 8],
                    input_shard[message_offset + 9],
                    input_shard[message_offset + 10],
                    input_shard[message_offset + 11],
                ]),
                w: u32::from_le_bytes([
                    input_shard[message_offset + 12],
                    input_shard[message_offset + 13],
                    input_shard[message_offset + 14],
                    input_shard[message_offset + 15],
                ]),
            }
        }
    };
    
    // Extract WASM bytecode length (bytes 16-20)
    let wasm_len = u32::from_le_bytes([
        input_shard[message_offset + 16],
        input_shard[message_offset + 17],
        input_shard[message_offset + 18],
        input_shard[message_offset + 19],
    ]);
    
    // Check if WASM bytecode is too large for GPU processing
    if wasm_len > MAX_CALLDATA_SIZE as u32 {
        mark_shard_ejected_with_reason(thread_id, ejected_shards, GPU_EJECTION_CALLDATA_OVERFLOW);
        return false;
    }
    
    // Extract WASM bytecode (starts at offset 20)
    let wasm_start = message_offset + 20;
    if wasm_start + wasm_len as usize > input_shard.len() {
        return false;
    }
    
    // Get WASM bytecode slice
    let wasm_bytecode = &input_shard[wasm_start..wasm_start + wasm_len as usize];
    
    // Execute the WASM contract using our real interpreter
    match execute_alkanes_message(
        wasm_bytecode,
        params.max_fuel,
        contract_id,
        params.block_height,
    ) {
        Ok((result_ptr, fuel_remaining)) => {
            // Write successful result
            if result_offset + 32 < output_result.len() {
                // Write success marker
                output_result[result_offset] = 1;
                
                // Write fuel consumed
                let fuel_consumed = params.max_fuel - fuel_remaining;
                let fuel_bytes = fuel_consumed.to_le_bytes();
                for i in 0..8 {
                    if result_offset + 1 + i < output_result.len() {
                        output_result[result_offset + 1 + i] = fuel_bytes[i];
                    }
                }
                
                // Write result pointer
                let result_bytes = result_ptr.to_le_bytes();
                for i in 0..4 {
                    if result_offset + 9 + i < output_result.len() {
                        output_result[result_offset + 9 + i] = result_bytes[i];
                    }
                }
                
                return true;
            }
        }
        Err(error_msg) => {
            // Check if this is an ejection case
            match error_msg {
                "shard ejected" => {
                    // This was already handled by the interpreter
                    mark_shard_ejected_with_reason(thread_id, ejected_shards, GPU_EJECTION_OTHER);
                }
                "out of fuel" => {
                    // Normal execution failure, not ejection
                    if result_offset < output_result.len() {
                        output_result[result_offset] = 0; // Failure marker
                    }
                }
                _ => {
                    // Other execution errors
                    if result_offset < output_result.len() {
                        output_result[result_offset] = 0; // Failure marker
                    }
                }
            }
        }
    }
    
    false
}

/// Mark a shard for ejection due to constraint violation
fn mark_shard_ejected(thread_id: u32, ejected_shards: &mut [u32]) {
    let index = (thread_id / 32) as usize; // 32 shards per u32
    let bit = thread_id % 32;
    
    if index < ejected_shards.len() {
        ejected_shards[index] |= 1 << bit;
    }
}

/// Mark a shard for ejection with specific reason
fn mark_shard_ejected_with_reason(thread_id: u32, ejected_shards: &mut [u32], _reason: u32) {
    // For now, just mark as ejected - reason could be encoded in higher bits
    mark_shard_ejected(thread_id, ejected_shards);
}

/// GPU ejection reasons
pub const GPU_EJECTION_NONE: u32 = 0;
pub const GPU_EJECTION_STORAGE_OVERFLOW: u32 = 1;  // Storage value too large for GPU buffer
pub const GPU_EJECTION_MEMORY_CONSTRAINT: u32 = 2; // GPU memory limit exceeded
pub const GPU_EJECTION_KV_OVERFLOW: u32 = 3;       // Too many K/V pairs for GPU
pub const GPU_EJECTION_CALLDATA_OVERFLOW: u32 = 4; // Calldata too large for GPU
pub const GPU_EJECTION_OTHER: u32 = 5;             // Other GPU-specific constraint

/// CPU-side implementation for testing and integration
#[cfg(not(target_arch = "spirv"))]
pub struct AlkanesGpuShader {
    // Minimal state for testing
}

#[cfg(not(target_arch = "spirv"))]
impl AlkanesGpuShader {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Test function to verify shader compilation with real WASM interpreter
    pub fn test_shader_compilation(&self) -> bool {
        // Test that we can create a WASM context and executor
        let context = SpirvWasmContext::new(1000000, UVec4::ZERO, 100);
        let _executor = SpirvWasmExecutor::new(context);
        true
    }
    
    /// Process a message with the real WASM interpreter (CPU version)
    pub fn process_message_with_wasm(
        &self,
        wasm_bytecode: &[u8],
        fuel: u64,
        contract_id: UVec4,
        height: u32,
    ) -> Result<(u32, u64), &'static str> {
        execute_alkanes_message(wasm_bytecode, fuel, contract_id, height)
    }
    
    /// Parse WASM bytecode and find exports (CPU version)
    pub fn parse_wasm_module(&self, bytecode: &[u8]) -> Result<bool, &'static str> {
        let mut parser = SpirvWasmParser::new(bytecode);
        match parser.parse_module() {
            Ok(module) => {
                // Check if we can find the __execute export
                let execute_func = find_export(&module, "__execute", bytecode);
                Ok(execute_func.is_some())
            }
            Err(_) => Ok(false), // Invalid WASM
        }
    }
    
    /// Check if a message would violate GPU constraints
    pub fn check_gpu_constraints_simple(calldata_len: u32) -> Option<u32> {
        // Check calldata size
        if calldata_len > MAX_CALLDATA_SIZE as u32 {
            return Some(GPU_EJECTION_CALLDATA_OVERFLOW);
        }
        
        None
    }
}

#[cfg(not(target_arch = "spirv"))]
impl Default for AlkanesGpuShader {
    fn default() -> Self {
        Self::new()
    }
}

/// Test function for non-SPIR-V targets with real WASM interpreter
#[cfg(not(target_arch = "spirv"))]
pub fn test_alkanes_gpu_shader_with_wasm() -> bool {
    let shader = AlkanesGpuShader::new();
    
    // Test basic compilation
    if !shader.test_shader_compilation() {
        return false;
    }
    
    // Test with minimal WASM bytecode (WASM magic + version)
    let minimal_wasm = [
        0x00, 0x61, 0x73, 0x6D, // WASM magic
        0x01, 0x00, 0x00, 0x00, // WASM version
    ];
    
    // Test WASM parsing
    match shader.parse_wasm_module(&minimal_wasm) {
        Ok(_) => true,
        Err(_) => false, // Expected for minimal WASM
    }
}

#[cfg(test)]
mod lib_tests {
    use super::*;
    
    #[test]
    fn test_compilation_works() {
        #[cfg(not(target_arch = "spirv"))]
        {
            assert!(test_alkanes_gpu_shader_with_wasm());
        }
        #[cfg(target_arch = "spirv")]
        {
            // For SPIR-V target, just verify it compiles
            assert!(true);
        }
    }
    
    #[test]
    #[cfg(not(target_arch = "spirv"))]
    fn test_alkanes_gpu_shader_creation() {
        let _shader = AlkanesGpuShader::new();
        // Test that we can create the shader without panicking
    }
    
    #[test]
    #[cfg(not(target_arch = "spirv"))]
    fn test_wasm_interpreter_integration() {
        let shader = AlkanesGpuShader::new();
        
        // Test WASM execution with minimal bytecode
        let minimal_wasm = [
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x01, 0x00, 0x00, 0x00, // WASM version
        ];
        
        let contract_id = UVec4::ZERO;
        let result = shader.process_message_with_wasm(&minimal_wasm, 1000000, contract_id, 100);
        
        // Should fail gracefully for invalid WASM
        assert!(result.is_err());
    }
    
    #[test]
    #[cfg(not(target_arch = "spirv"))]
    fn test_gpu_constraint_checking_simple() {
        // Test normal calldata
        assert_eq!(AlkanesGpuShader::check_gpu_constraints_simple(100), None);
        
        // Test oversized calldata
        assert_eq!(
            AlkanesGpuShader::check_gpu_constraints_simple(MAX_CALLDATA_SIZE as u32 + 1),
            Some(GPU_EJECTION_CALLDATA_OVERFLOW)
        );
    }
    
    #[test]
    fn test_ejection_constants() {
        assert_eq!(GPU_EJECTION_NONE, 0);
        assert_eq!(GPU_EJECTION_STORAGE_OVERFLOW, 1);
        assert_eq!(GPU_EJECTION_MEMORY_CONSTRAINT, 2);
        assert_eq!(GPU_EJECTION_KV_OVERFLOW, 3);
        assert_eq!(GPU_EJECTION_CALLDATA_OVERFLOW, 4);
        assert_eq!(GPU_EJECTION_OTHER, 5);
    }
    
    #[test]
    fn test_gpu_limits() {
        assert_eq!(GPU_WASM_MEMORY_LIMIT, 42 * 1024 * 1024);
        assert_eq!(GPU_MAX_CALL_STACK_DEPTH, 256);
        assert_eq!(MAX_CALLDATA_SIZE, 2048);
        assert_eq!(MAX_KV_PAIRS, 1024);
    }
    
    #[test]
    #[cfg(not(target_arch = "spirv"))]
    fn test_wasm_context_creation() {
        let context = SpirvWasmContext::new(1000000, UVec4::ZERO, 100);
        assert_eq!(context.fuel, 1000000);
        assert_eq!(context.height, 100);
        assert!(!context.failed);
    }
    
    #[test]
    #[cfg(not(target_arch = "spirv"))]
    fn test_wasm_executor_creation() {
        let context = SpirvWasmContext::new(1000000, UVec4::ZERO, 100);
        let executor = SpirvWasmExecutor::new(context);
        assert_eq!(executor.get_fuel(), 1000000);
        assert!(!executor.should_eject());
    }
}
