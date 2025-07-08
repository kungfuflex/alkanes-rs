//! Standalone tests for alkanes-gpu-shader
//! 
//! These tests verify our SPIR-V-compatible implementation works correctly
//! without requiring the full alkanes-wasmi dependency chain.

#[cfg(test)]
mod standalone_tests {
    use crate::{
        AlkanesGpuShader, UVec4, MAX_CALLDATA_SIZE,
        GPU_EJECTION_NONE, GPU_EJECTION_CALLDATA_OVERFLOW,
        GPU_EJECTION_STORAGE_OVERFLOW, GPU_EJECTION_MEMORY_CONSTRAINT,
        GPU_EJECTION_KV_OVERFLOW, GPU_EJECTION_OTHER
    };
    use crate::wasm_interpreter::{SpirvWasmContext, SpirvWasmExecutor, execute_alkanes_message};

    #[test]
    fn test_basic_wasm_context() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        
        assert_eq!(context.fuel, 1000000);
        assert_eq!(context.height, 100);
        assert!(!context.failed);
    }

    #[test]
    fn test_fuel_consumption() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let mut context = SpirvWasmContext::new(1000, contract_id, 100);
        
        // Test fuel consumption
        assert!(context.consume_fuel(500));
        assert_eq!(context.fuel, 500);
        
        // Test fuel exhaustion
        assert!(!context.consume_fuel(600));
        assert!(context.failed);
    }

    #[test]
    fn test_wasm_executor_basic() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let executor = SpirvWasmExecutor::new(context);
        
        assert_eq!(executor.get_fuel(), 1000000);
        assert!(!executor.should_eject());
    }

    #[test]
    fn test_memory_bounds_checking() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test valid memory access
        assert!(executor.store_i32(0, 42).is_ok());
        assert_eq!(executor.load_i32(0).unwrap(), 42);
        
        // Test out of bounds access with known large addresses
        // Default memory size is 65536 (64KB), so these should fail
        assert!(executor.load_i32(65536).is_err());
        assert!(executor.load_i32(100000).is_err());
    }

    #[test]
    fn test_constraint_checking() {
        // Test calldata size constraint
        assert_eq!(
            AlkanesGpuShader::check_gpu_constraints_simple(100),
            None
        );
        
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
    fn test_shader_creation() {
        let shader = AlkanesGpuShader::new();
        assert!(shader.test_shader_compilation());
    }

    #[test]
    fn test_minimal_wasm_execution() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        
        // Minimal WASM with just magic and version
        let minimal_wasm = vec![
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x01, 0x00, 0x00, 0x00, // WASM version
        ];
        
        let result = execute_alkanes_message(&minimal_wasm, 1000000, contract_id, 100);
        
        // Should fail gracefully for incomplete WASM
        assert!(result.is_err());
    }
}