//! Tests for alkanes-gpu-shader with real WASM interpreter
//! 
//! These tests verify our SPIR-V-compatible implementation works correctly
//! without requiring external dependencies.

#[cfg(test)]
mod tests {
    use crate::{
        AlkanesGpuShader, UVec4, MAX_CALLDATA_SIZE,
        GPU_EJECTION_NONE, GPU_EJECTION_CALLDATA_OVERFLOW,
        GPU_EJECTION_STORAGE_OVERFLOW, GPU_EJECTION_MEMORY_CONSTRAINT,
        GPU_EJECTION_KV_OVERFLOW, GPU_EJECTION_OTHER
    };
    use crate::wasm_interpreter::{
        SpirvWasmContext, SpirvWasmExecutor, execute_alkanes_message, WasmValue,
    };
    use crate::wasm_parser::SpirvWasmParser;

    /// Create minimal valid WASM bytecode for testing
    fn create_minimal_wasm_bytecode() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x01, 0x00, 0x00, 0x00, // WASM version
            // Type section
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            // Function section  
            0x03, 0x02, 0x01, 0x00,
            // Code section
            0x0A, 0x04, 0x01, 0x02, 0x00, 0x0B,
        ]
    }

    #[test]
    fn test_wasm_context_creation() {
        let contract_id = UVec4 { x: 1, y: 2, z: 3, w: 4 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        
        assert_eq!(context.fuel, 1000000);
        assert_eq!(context.height, 100);
        assert!(!context.failed);
        assert_eq!(context.call_depth, 0);
    }

    #[test]
    fn test_wasm_executor_creation() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let executor = SpirvWasmExecutor::new(context);
        
        assert_eq!(executor.get_fuel(), 1000000);
        assert!(!executor.should_eject());
        assert_eq!(executor.get_memory_size(), 65536); // 1 page
    }

    #[test]
    fn test_wasm_stack_operations() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test stack push/pop
        executor.test_push_value(WasmValue::I32(42)).unwrap();
        executor.test_push_value(WasmValue::I32(24)).unwrap();
        
        let val2 = executor.test_pop_value().unwrap();
        let val1 = executor.test_pop_value().unwrap();
        
        assert_eq!(val1.as_i32(), Some(42));
        assert_eq!(val2.as_i32(), Some(24));
    }

    #[test]
    fn test_wasm_memory_operations() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test memory store/load
        executor.store_i32(0, 0x12345678).unwrap();
        let value = executor.load_i32(0).unwrap();
        assert_eq!(value, 0x12345678);
    }

    #[test]
    fn test_host_function_fuel() {
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        let context = SpirvWasmContext::new(1000000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Push output pointer for fuel function
        executor.test_push_value(WasmValue::I32(1000)).unwrap();
        
        let initial_fuel = executor.get_fuel();
        executor.test_call_host_function(crate::wasm_interpreter::host_functions::FUEL).unwrap();
        
        // Should have consumed fuel
        assert!(executor.get_fuel() < initial_fuel);
    }

    #[test]
    fn test_execute_alkanes_message() {
        let bytecode = create_minimal_wasm_bytecode();
        let contract_id = UVec4 { x: 2, y: 0, z: 0, w: 0 };
        
        let result = execute_alkanes_message(&bytecode, 1000000, contract_id, 100);
        
        // Should fail gracefully for minimal WASM (no __execute function)
        assert!(result.is_err());
    }

    #[test]
    fn test_wasm_parser_creation() {
        let bytecode = create_minimal_wasm_bytecode();
        let mut parser = SpirvWasmParser::new(&bytecode);
        
        // Test magic number check
        assert!(parser.test_check_magic().unwrap());
        
        // Reset parser for version check
        let mut parser = SpirvWasmParser::new(&bytecode[4..]);
        assert!(parser.test_check_version().unwrap());
    }

    #[test]
    fn test_fuel_costs_compatibility() {
        use crate::wasm_interpreter::fuel_costs;
        
        // Verify fuel costs match expected values
        assert_eq!(fuel_costs::PER_REQUEST_BYTE, 1);
        assert_eq!(fuel_costs::PER_LOAD_BYTE, 2);
        assert_eq!(fuel_costs::PER_STORE_BYTE, 40);
        assert_eq!(fuel_costs::SEQUENCE, 5);
    }

    #[test]
    fn test_host_function_ids() {
        use crate::wasm_interpreter::host_functions;
        
        // Verify host function IDs are correct
        assert_eq!(host_functions::ABORT, 0);
        assert_eq!(host_functions::LOAD_STORAGE, 1);
        assert_eq!(host_functions::REQUEST_STORAGE, 2);
        assert_eq!(host_functions::LOG, 3);
        assert_eq!(host_functions::FUEL, 8);
        assert_eq!(host_functions::HEIGHT, 9);
    }

    #[test]
    fn test_wasm_module_parsing() {
        let bytecode = create_minimal_wasm_bytecode();
        let mut parser = SpirvWasmParser::new(&bytecode);
        
        // Should be able to parse the module structure
        let result = parser.parse_module();
        assert!(result.is_ok());
        
        let module = result.unwrap();
        assert!(module.func_types_len > 0);
    }

    #[test]
    fn test_gpu_shader_integration() {
        let shader = AlkanesGpuShader::new();
        
        // Test basic functionality
        assert!(shader.test_shader_compilation());
        
        // Test WASM processing
        let bytecode = create_minimal_wasm_bytecode();
        let contract_id = UVec4 { x: 1, y: 2, z: 3, w: 4 };
        let result = shader.process_message_with_wasm(&bytecode, 1000000, contract_id, 100);
        
        // Should fail gracefully for minimal WASM
        assert!(result.is_err());
        
        // Test WASM parsing
        let parse_result = shader.parse_wasm_module(&bytecode);
        assert!(parse_result.is_ok());
    }
}