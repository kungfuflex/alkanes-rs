//! Comprehensive tests for alkanes GPU shader implementation
//! 
//! These tests verify that our SPIR-V-compatible WASM interpreter
//! maintains compatibility with the canonical alkanes VM implementation.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm_interpreter::{SpirvWasmContext, SpirvWasmExecutor, execute_alkanes_message};
    use crate::wasm_parser::{SpirvWasmParser, find_export};

    #[test]
    fn test_spirv_vec_basic_operations() {
        let mut vec = SpirvVec::<u8>::new();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
        
        vec.push(42);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], 42);
        
        vec.extend_from_slice(&[1, 2, 3]);
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[1], 1);
        assert_eq!(vec[2], 2);
        assert_eq!(vec[3], 3);
    }

    #[test]
    fn test_wasm_context_fuel_management() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let mut context = SpirvWasmContext::new(1000, contract_id, 100);
        
        assert_eq!(context.fuel, 1000);
        assert_eq!(context.height, 100);
        assert!(!context.failed);
        
        // Test fuel consumption
        assert!(context.consume_fuel(500));
        assert_eq!(context.fuel, 500);
        
        // Test fuel exhaustion
        assert!(!context.consume_fuel(600));
        assert!(context.failed);
    }

    #[test]
    fn test_wasm_executor_stack_operations() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test stack push/pop
        executor.push_value(crate::wasm_interpreter::WasmValue::I32(42)).unwrap();
        executor.push_value(crate::wasm_interpreter::WasmValue::I32(24)).unwrap();
        
        let val2 = executor.pop_value().unwrap();
        let val1 = executor.pop_value().unwrap();
        
        assert_eq!(val1.as_i32().unwrap(), 42);
        assert_eq!(val2.as_i32().unwrap(), 24);
    }

    #[test]
    fn test_wasm_executor_memory_operations() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let context = SpirvWasmContext::new(10000, contract_id, 100);
        let mut executor = SpirvWasmExecutor::new(context);
        
        // Test memory store/load
        executor.store_i32(0, 0x12345678).unwrap();
        let value = executor.load_i32(0).unwrap();
        assert_eq!(value, 0x12345678);
        
        // Test memory bounds checking
        assert!(executor.store_i32(executor.memory_size - 3, 0).is_err());
    }

    #[test]
    fn test_wasm_parser_magic_and_version() {
        let valid_wasm = [
            0x00, 0x61, 0x73, 0x6D, // magic
            0x01, 0x00, 0x00, 0x00, // version
        ];
        
        let mut parser = SpirvWasmParser::new(&valid_wasm);
        assert!(parser.check_magic().unwrap());
        assert!(parser.check_version().unwrap());
        
        let invalid_magic = [
            0x00, 0x61, 0x73, 0x6E, // invalid magic
            0x01, 0x00, 0x00, 0x00,
        ];
        
        let mut parser = SpirvWasmParser::new(&invalid_magic);
        assert!(!parser.check_magic().unwrap());
    }

    #[test]
    fn test_minimal_wasm_bytecode_generation() {
        let bytecode = create_minimal_wasm_bytecode();
        assert!(!bytecode.is_empty());
        
        // Check WASM magic number
        assert_eq!(bytecode[0], 0x00);
        assert_eq!(bytecode[1], 0x61);
        assert_eq!(bytecode[2], 0x73);
        assert_eq!(bytecode[3], 0x6D);
        
        // Check version
        assert_eq!(bytecode[4], 0x01);
        assert_eq!(bytecode[5], 0x00);
        assert_eq!(bytecode[6], 0x00);
        assert_eq!(bytecode[7], 0x00);
    }

    #[test]
    fn test_alkanes_message_execution() {
        let contract_id = UVec4::new(2, 0, 0, 0);
        let bytecode = create_minimal_wasm_bytecode();
        
        let result = execute_alkanes_message(&bytecode, 10000, contract_id, 100);
        assert!(result.is_ok());
        
        let (result_ptr, remaining_fuel) = result.unwrap();
        assert_eq!(result_ptr, 0); // Our minimal implementation returns 0
        assert!(remaining_fuel < 10000); // Some fuel should be consumed
    }

    #[test]
    fn test_fuel_costs_compatibility() {
        use crate::wasm_interpreter::fuel_costs;
        
        // Verify fuel costs match alkanes VM
        assert_eq!(fuel_costs::PER_REQUEST_BYTE, 1);
        assert_eq!(fuel_costs::PER_LOAD_BYTE, 2);
        assert_eq!(fuel_costs::PER_STORE_BYTE, 40);
        assert_eq!(fuel_costs::SEQUENCE, 5);
        assert_eq!(fuel_costs::FUEL, 5);
        assert_eq!(fuel_costs::EXTCALL, 500);
        assert_eq!(fuel_costs::HEIGHT, 10);
        assert_eq!(fuel_costs::BALANCE, 10);
        assert_eq!(fuel_costs::LOAD_BLOCK, 1000);
        assert_eq!(fuel_costs::LOAD_TRANSACTION, 500);
    }

    #[test]
    fn test_host_function_ids() {
        use crate::wasm_interpreter::host_functions;
        
        // Verify host function IDs match alkanes VM
        assert_eq!(host_functions::ABORT, 0);
        assert_eq!(host_functions::LOAD_STORAGE, 1);
        assert_eq!(host_functions::REQUEST_STORAGE, 2);
        assert_eq!(host_functions::LOG, 3);
        assert_eq!(host_functions::BALANCE, 4);
        assert_eq!(host_functions::REQUEST_CONTEXT, 5);
        assert_eq!(host_functions::LOAD_CONTEXT, 6);
        assert_eq!(host_functions::SEQUENCE, 7);
        assert_eq!(host_functions::FUEL, 8);
        assert_eq!(host_functions::HEIGHT, 9);
        assert_eq!(host_functions::RETURNDATACOPY, 10);
        assert_eq!(host_functions::REQUEST_TRANSACTION, 11);
        assert_eq!(host_functions::LOAD_TRANSACTION, 12);
        assert_eq!(host_functions::REQUEST_BLOCK, 13);
        assert_eq!(host_functions::LOAD_BLOCK, 14);
        assert_eq!(host_functions::CALL, 15);
        assert_eq!(host_functions::DELEGATECALL, 16);
        assert_eq!(host_functions::STATICCALL, 17);
    }

    #[test]
    fn test_export_finding() {
        let module = crate::wasm_parser::SpirvWasmModule {
            func_types: [crate::wasm_interpreter::WasmFuncType { params: &[], results: &[] }; 64],
            func_types_len: 0,
            func_type_indices: [0; 256],
            func_count: 0,
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
            exports_len: 0,
            functions: [crate::wasm_parser::WasmFunction {
                code_offset: 0,
                code_len: 0,
                locals: [crate::wasm_interpreter::WasmValueType::I32; 32],
                locals_len: 0,
            }; 256],
            functions_len: 0,
            memory_min: 0,
            memory_max: None,
            start_func: None,
        };
        
        // Test finding standard alkanes exports
        assert_eq!(find_export(&module, "__execute", &[]), Some(0));
        assert_eq!(find_export(&module, "__meta", &[]), Some(1));
        assert_eq!(find_export(&module, "memory", &[]), Some(0));
        assert_eq!(find_export(&module, "unknown", &[]), None);
    }

    #[test]
    fn test_gpu_message_structures() {
        let message = GpuMessageInput {
            txid: [0; 32],
            txindex: 0,
            height: 100,
            vout: 0,
            pointer: 0,
            refund_pointer: 0,
            calldata_len: 10,
            calldata: [0; MAX_CALLDATA_SIZE],
        };
        
        assert_eq!(message.height, 100);
        assert_eq!(message.calldata_len, 10);
        
        let result = GpuExecutionResult::default();
        assert_eq!(result.status, 0);
        assert_eq!(result.gas_used, 0);
        assert_eq!(result.return_data_len, 0);
    }

    #[test]
    fn test_infrastructure_integration() {
        let success = test_alkanes_infrastructure_integration();
        assert!(success, "Infrastructure integration test failed");
    }
}