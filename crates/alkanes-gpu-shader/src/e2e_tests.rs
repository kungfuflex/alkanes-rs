//! End-to-end tests for alkanes GPU shader WASM interpreter
//! 
//! These tests verify that our SPIR-V-compatible implementation maintains
//! exact compatibility with the canonical alkanes VM behavior.

#[cfg(test)]
mod e2e_tests {
    use super::*;
    use crate::wasm_interpreter::{
        SpirvWasmContext, SpirvWasmExecutor, execute_alkanes_message,
        WasmValue, WasmOpcode, fuel_costs, host_functions
    };
    use crate::wasm_parser::{SpirvWasmParser, find_export};

    /// Test that our WASM interpreter can execute a complete alkanes contract
    #[test]
    fn test_complete_contract_execution() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let bytecode = create_test_contract_bytecode();
        let initial_fuel = 100000;
        
        let result = execute_alkanes_message(&bytecode, initial_fuel, contract_id, 100);
        assert!(result.is_ok(), "Contract execution should succeed");
        
        let (result_ptr, remaining_fuel) = result.unwrap();
        assert!(remaining_fuel < initial_fuel, "Some fuel should be consumed");
        assert!(remaining_fuel > 0, "Should not consume all fuel");
    }

    /// Test fuel metering accuracy against known operations
    #[test]
    fn test_fuel_metering_accuracy() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        let initial_fuel = executor.get_fuel();
        
        // Test basic instruction fuel consumption
        let result = executor.execute_instruction(WasmOpcode::I32Const(42));
        assert!(result.is_ok());
        assert_eq!(executor.get_fuel(), initial_fuel - 1, "Basic instruction should consume 1 fuel");
        
        // Test memory operation fuel consumption
        executor.push_value(WasmValue::I32(0)).unwrap(); // address
        let fuel_before_store = executor.get_fuel();
        let result = executor.execute_instruction(WasmOpcode::I32Store(0, 0));
        assert!(result.is_ok());
        let fuel_after_store = executor.get_fuel();
        let store_fuel_consumed = fuel_before_store - fuel_after_store;
        assert_eq!(store_fuel_consumed, 1 + fuel_costs::PER_STORE_BYTE * 4, 
                   "Store should consume instruction fuel + store fuel");
    }

    /// Test host function fuel consumption
    #[test]
    fn test_host_function_fuel_consumption() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test FUEL host function
        executor.push_value(WasmValue::I32(1000)).unwrap(); // output pointer
        let fuel_before = executor.get_fuel();
        let result = executor.call_host_function(host_functions::FUEL);
        assert!(result.is_ok());
        let fuel_after = executor.get_fuel();
        assert_eq!(fuel_before - fuel_after, 1 + fuel_costs::FUEL, 
                   "FUEL host function should consume correct fuel");
        
        // Test HEIGHT host function
        executor.push_value(WasmValue::I32(1004)).unwrap(); // output pointer
        let fuel_before = executor.get_fuel();
        let result = executor.call_host_function(host_functions::HEIGHT);
        assert!(result.is_ok());
        let fuel_after = executor.get_fuel();
        assert_eq!(fuel_before - fuel_after, 1 + fuel_costs::HEIGHT,
                   "HEIGHT host function should consume correct fuel");
    }

    /// Test memory bounds checking and safety
    #[test]
    fn test_memory_bounds_safety() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test valid memory access
        assert!(executor.store_i32(0, 0x12345678).is_ok());
        assert_eq!(executor.load_i32(0).unwrap(), 0x12345678);
        
        // Test out-of-bounds access
        let memory_size = executor.memory_size;
        assert!(executor.store_i32(memory_size - 3, 0).is_err(), 
                "Should reject out-of-bounds store");
        assert!(executor.load_i32(memory_size - 3).is_err(),
                "Should reject out-of-bounds load");
    }

    /// Test stack overflow protection
    #[test]
    fn test_stack_overflow_protection() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Fill the stack to capacity
        for i in 0..1024 {
            let result = executor.push_value(WasmValue::I32(i as i32));
            if i < 1023 {
                assert!(result.is_ok(), "Should be able to push {} values", i + 1);
            }
        }
        
        // Try to overflow
        let result = executor.push_value(WasmValue::I32(1024));
        assert!(result.is_err(), "Should reject stack overflow");
    }

    /// Test fuel exhaustion handling
    #[test]
    fn test_fuel_exhaustion() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(5, contract_id, 100); // Very low fuel
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Execute instructions until fuel runs out
        let mut instructions_executed = 0;
        loop {
            let result = executor.execute_instruction(WasmOpcode::I32Const(42));
            if result.is_err() {
                break;
            }
            instructions_executed += 1;
            if instructions_executed > 10 {
                panic!("Should have run out of fuel by now");
            }
        }
        
        assert!(executor.should_eject(), "Executor should be marked for ejection");
        assert_eq!(executor.get_fuel(), 0, "Fuel should be exhausted");
    }

    /// Test WASM parser with various bytecode formats
    #[test]
    fn test_wasm_parser_robustness() {
        // Test valid minimal WASM
        let valid_wasm = create_minimal_wasm_bytecode();
        let mut parser = SpirvWasmParser::new(&valid_wasm);
        let result = parser.parse_module();
        assert!(result.is_ok(), "Should parse valid WASM");
        
        // Test invalid magic number
        let invalid_magic = [0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00];
        let mut parser = SpirvWasmParser::new(&invalid_magic);
        let result = parser.parse_module();
        assert!(result.is_err(), "Should reject invalid magic");
        
        // Test truncated bytecode
        let truncated = [0x00, 0x61, 0x73]; // Incomplete magic
        let mut parser = SpirvWasmParser::new(&truncated);
        let result = parser.parse_module();
        assert!(result.is_err(), "Should reject truncated bytecode");
    }

    /// Test instruction parsing and execution
    #[test]
    fn test_instruction_parsing_and_execution() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test arithmetic operations
        executor.push_value(WasmValue::I32(10)).unwrap();
        executor.push_value(WasmValue::I32(5)).unwrap();
        
        let result = executor.execute_instruction(WasmOpcode::I32Add);
        assert!(result.is_ok());
        
        let result_val = executor.pop_value().unwrap();
        assert_eq!(result_val.as_i32().unwrap(), 15, "10 + 5 should equal 15");
        
        // Test comparison operations
        executor.push_value(WasmValue::I32(10)).unwrap();
        executor.push_value(WasmValue::I32(10)).unwrap();
        
        let result = executor.execute_instruction(WasmOpcode::I32Eq);
        assert!(result.is_ok());
        
        let result_val = executor.pop_value().unwrap();
        assert_eq!(result_val.as_i32().unwrap(), 1, "10 == 10 should be true (1)");
    }

    /// Test local variable operations
    #[test]
    fn test_local_variable_operations() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Set a local variable
        executor.push_value(WasmValue::I32(42)).unwrap();
        let result = executor.execute_instruction(WasmOpcode::LocalSet(0));
        assert!(result.is_ok());
        
        // Get the local variable
        let result = executor.execute_instruction(WasmOpcode::LocalGet(0));
        assert!(result.is_ok());
        
        let value = executor.pop_value().unwrap();
        assert_eq!(value.as_i32().unwrap(), 42, "Local variable should retain value");
        
        // Test out-of-bounds local access
        let result = executor.execute_instruction(WasmOpcode::LocalGet(256));
        assert!(result.is_err(), "Should reject out-of-bounds local access");
    }

    /// Test export finding functionality
    #[test]
    fn test_export_finding() {
        let module = create_test_module();
        let bytecode = create_minimal_wasm_bytecode();
        
        // Test finding standard alkanes exports
        assert_eq!(find_export(&module, "__execute", &bytecode), Some(0));
        assert_eq!(find_export(&module, "__meta", &bytecode), Some(1));
        assert_eq!(find_export(&module, "memory", &bytecode), Some(0));
        assert_eq!(find_export(&module, "nonexistent", &bytecode), None);
    }

    /// Test GPU message processing pipeline
    #[test]
    fn test_gpu_message_processing() {
        let params = ShaderParams {
            message_count: 1,
            block_height: 100,
            base_fuel: 10000,
            max_fuel: 50000,
        };
        
        // Create a test message buffer
        let mut messages = vec![0u8; 1024];
        
        // Set up contract ID in message (first 16 bytes)
        messages[0..4].copy_from_slice(&2u32.to_le_bytes()); // block
        messages[4..8].copy_from_slice(&0u32.to_le_bytes()); // tx
        messages[8..12].copy_from_slice(&0u32.to_le_bytes()); // reserved
        messages[12..16].copy_from_slice(&0u32.to_le_bytes()); // reserved
        
        let result = process_alkanes_message(0, &messages, &params);
        assert!(result.is_ok(), "Message processing should succeed");
        
        let response_data = result.unwrap();
        assert!(!response_data.is_empty(), "Should return response data");
    }

    /// Test serialization and deserialization
    #[test]
    fn test_response_serialization() {
        let response = AlkanesResponse {
            success: true,
            fuel_consumed: 1000,
            result_data: 0x12345678,
        };
        
        let serialized = serialize_response(&response);
        assert!(serialized.is_ok());
        
        let data = serialized.unwrap();
        assert_eq!(data[0], 1, "Success flag should be 1");
        
        // Check fuel consumed (8 bytes)
        let fuel_bytes = &data[1..9];
        let fuel = u64::from_le_bytes([
            fuel_bytes[0], fuel_bytes[1], fuel_bytes[2], fuel_bytes[3],
            fuel_bytes[4], fuel_bytes[5], fuel_bytes[6], fuel_bytes[7],
        ]);
        assert_eq!(fuel, 1000, "Fuel consumed should be serialized correctly");
        
        // Check result data (4 bytes)
        let result_bytes = &data[9..13];
        let result = u32::from_le_bytes([
            result_bytes[0], result_bytes[1], result_bytes[2], result_bytes[3],
        ]);
        assert_eq!(result, 0x12345678, "Result data should be serialized correctly");
    }

    /// Test SpirvVec operations thoroughly
    #[test]
    fn test_spirv_vec_comprehensive() {
        let mut vec = SpirvVec::<u32>::new();
        
        // Test basic operations
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
        
        // Test pushing elements
        for i in 0..100 {
            vec.push(i);
            assert_eq!(vec.len(), i as usize + 1);
            assert_eq!(vec[i as usize], i);
        }
        
        // Test extend_from_slice
        let additional = [100, 101, 102, 103, 104];
        vec.extend_from_slice(&additional);
        assert_eq!(vec.len(), 105);
        assert_eq!(vec[100], 100);
        assert_eq!(vec[104], 104);
        
        // Test capacity limits
        let mut large_vec = SpirvVec::<u8>::new();
        for i in 0..1024 {
            large_vec.push(i as u8);
        }
        assert_eq!(large_vec.len(), 1024);
        
        // Try to exceed capacity
        large_vec.push(255);
        assert_eq!(large_vec.len(), 1024, "Should not exceed capacity");
    }

    // Helper functions for tests

    fn create_test_contract_bytecode() -> SpirvVec<u8> {
        // Create a more complex test contract that exercises various operations
        let mut bytecode = create_minimal_wasm_bytecode();
        
        // Add some additional test instructions
        bytecode.extend_from_slice(&[
            0x41, 0x2A, // i32.const 42
            0x20, 0x00, // local.get 0
            0x6A,       // i32.add
            0x21, 0x01, // local.set 1
            0x0F,       // return
        ]);
        
        bytecode
    }

    fn create_test_module() -> crate::wasm_parser::SpirvWasmModule {
        crate::wasm_parser::SpirvWasmModule {
            func_types: [crate::wasm_interpreter::WasmFuncType { params: &[], results: &[] }; 64],
            func_types_len: 1,
            func_type_indices: [0; 256],
            func_count: 2,
            imports: [crate::wasm_parser::WasmImport {
                module_name_ptr: 0,
                module_name_len: 0,
                field_name_ptr: 0,
                field_name_len: 0,
                kind: crate::wasm_parser::ImportKind::Function(0),
            }; 64],
            imports_len: 0,
            exports: [crate::wasm_parser::WasmExport {
                name_ptr: 0,
                name_len: 0,
                kind: crate::wasm_parser::ExportKind::Function(0),
            }; 64],
            exports_len: 2,
            functions: [crate::wasm_parser::WasmFunction {
                code_offset: 0,
                code_len: 0,
                locals: [crate::wasm_interpreter::WasmValueType::I32; 32],
                locals_len: 0,
            }; 256],
            functions_len: 2,
            memory_min: 1,
            memory_max: Some(10),
            start_func: None,
        }
    }
}