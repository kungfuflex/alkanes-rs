use super::*;
use crate::vm::{AlkanesExecutor, IndexerError, ValidationLayer};
use std::sync::{Arc, Mutex};
use wasmi::*;

#[test]
fn test_wasm_size_validation() {
    let validation = ValidationLayer::default();
    let binary = vec![0u8; MAX_WASM_SIZE + 1];
    let engine = Engine::default();
    
    match validation.wasm_validator.validate_module(&binary, &engine) {
        Err(IndexerError::WasmValidation(msg)) => {
            assert!(msg.contains("WASM module too large"));
        }
        _ => panic!("Expected WasmValidation error"),
    }
}

#[test]
fn test_memory_validation() {
    let validation = ValidationLayer::default();
    let mem_validator = &validation.memory_validator;

    // Create test memory
    let engine = Engine::default();
    let memory_ty = MemoryType::new(1, Some(1));
    let memory = Memory::new(&engine, memory_ty).unwrap();

    // Test out of bounds access
    assert!(mem_validator
        .validate_memory_access(&memory, 65536, 1)
        .is_err());

    // Test negative offset
    assert!(mem_validator.validate_memory_access(&memory, -1, 1).is_err());

    // Test valid access
    assert!(mem_validator
        .validate_memory_access(&memory, 0, 100)
        .is_ok());
}

#[test]
fn test_resource_tracking() {
    let mut tracker = ResourceTracker::new();

    // Test memory tracking
    assert!(tracker.track_memory_allocation(1000).is_ok());
    assert!(tracker
        .track_memory_allocation(MAX_MEMORY_SIZE + 1)
        .is_err());

    // Test instruction tracking
    assert!(tracker.track_instruction(100).is_ok());
    tracker.record_error();
    assert_eq!(tracker.error_count, 1);
}

#[test]
fn test_safe_executor() {
    let binary = include_bytes!("../test_data/simple.wasm");
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));
    
    let result = AlkanesExecutor::new(binary, context.clone(), 1000000);
    assert!(result.is_ok());

    let mut executor = result.unwrap();
    let response = executor.execute();
    
    match response {
        Ok(_) => {}
        Err(e) => panic!("Execution failed: {:?}", e),
    }
}

#[test]
fn test_invalid_wasm() {
    let invalid_wasm = vec![0, 1, 2, 3]; // Invalid WASM binary
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));
    
    let result = AlkanesExecutor::new(&invalid_wasm, context.clone(), 1000000);
    assert!(matches!(result, Err(IndexerError::WasmValidation(_))));
}

#[test]
fn test_memory_limits() {
    let binary = include_bytes!("../test_data/memory_hungry.wasm");
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));
    
    let result = AlkanesExecutor::new(binary, context.clone(), 1000000);
    assert!(result.is_ok());

    let mut executor = result.unwrap();
    let response = executor.execute();
    
    assert!(matches!(response, Err(IndexerError::ResourceExhausted(_))));
}

#[test]
fn test_fuel_limits() {
    let binary = include_bytes!("../test_data/infinite_loop.wasm");
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::default()));
    
    let result = AlkanesExecutor::new(binary, context.clone(), 1000);
    assert!(result.is_ok());

    let mut executor = result.unwrap();
    let response = executor.execute();
    
    assert!(matches!(response, Err(IndexerError::Fuel(_))));
}