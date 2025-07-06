//! Generic host functions trait for alkanes WASM execution
//! 
//! This module provides a generic trait for host functions that can work with
//! any KeyValuePointer implementation, enabling both CPU and GPU execution
//! with different storage backends.

use crate::vm::{GenericAlkanesRuntimeContext, GenericMessageContextParcel};
use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::{Arc, Mutex};
use wasmi::{Caller, Linker};

/// Result of a host function call that may trigger ejection
#[derive(Debug, Clone)]
pub enum HostFunctionResult<T> {
    /// Operation completed successfully
    Success(T),
    /// Operation failed with a normal WASM error (execution can continue)
    WasmError(String),
    /// Operation triggered GPU constraint violation (eject entire shard)
    Ejected(u32), // ejection reason code
}

impl<T> HostFunctionResult<T> {
    pub fn is_ejected(&self) -> bool {
        matches!(self, HostFunctionResult::Ejected(_))
    }
    
    pub fn ejection_reason(&self) -> Option<u32> {
        match self {
            HostFunctionResult::Ejected(reason) => Some(*reason),
            _ => None,
        }
    }
}

/// Generic trait for host functions that work with any KeyValuePointer backend
pub trait GenericHostFunctions<KV: KeyValuePointer + Clone> {
    /// Request storage allocation size for a key
    fn request_storage(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
    ) -> HostFunctionResult<i32>;
    
    /// Load storage value for a key
    fn load_storage(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
    ) -> HostFunctionResult<Vec<u8>>;
    
    /// Store a value at a key
    fn store_value(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
        value: &[u8],
    ) -> HostFunctionResult<()>;
    
    /// Request context data size
    fn request_context(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32>;
    
    /// Load context data
    fn load_context(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>>;
    
    /// Log a message
    fn log(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        message: &[u8],
    ) -> HostFunctionResult<()>;
    
    /// Get balance for who/what pair
    fn balance(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        who: &[u8],
        what: &[u8],
    ) -> HostFunctionResult<Vec<u8>>;
    
    /// Get current fuel remaining
    fn fuel(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u64>;
    
    /// Get current block height
    fn height(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u64>;
    
    /// Get sequence number
    fn sequence(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u128>;
    
    /// Request transaction data size
    fn request_transaction(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32>;
    
    /// Load transaction data
    fn load_transaction(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>>;
    
    /// Request block data size
    fn request_block(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32>;
    
    /// Load block data
    fn load_block(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>>;
    
    /// Copy return data from previous call
    fn returndatacopy(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>>;
}

/// CPU implementation of host functions (no ejection, full access)
pub struct CpuHostFunctions;

impl<KV: KeyValuePointer + Clone> GenericHostFunctions<KV> for CpuHostFunctions {
    fn request_storage(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
    ) -> HostFunctionResult<i32> {
        let context_guard = context.lock().unwrap();
        let myself = context_guard.myself.clone();
        let storage_key = context_guard.message.atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key.to_vec());
        let size = storage_key.get().len() as i32;
        HostFunctionResult::Success(size)
    }
    
    fn load_storage(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
    ) -> HostFunctionResult<Vec<u8>> {
        let context_guard = context.lock().unwrap();
        let myself = context_guard.myself.clone();
        let storage_key = context_guard.message.atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key.to_vec());
        let value = storage_key.get();
        HostFunctionResult::Success(value.as_ref().clone())
    }
    
    fn store_value(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        key: &[u8],
        value: &[u8],
    ) -> HostFunctionResult<()> {
        let mut context_guard = context.lock().unwrap();
        let myself = context_guard.myself.clone();
        let mut storage_key = context_guard.message.atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key.to_vec());
        storage_key.set(Arc::new(value.to_vec()));
        HostFunctionResult::Success(())
    }
    
    fn request_context(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32> {
        let context_guard = context.lock().unwrap();
        let serialized = context_guard.serialize();
        HostFunctionResult::Success(serialized.len() as i32)
    }
    
    fn load_context(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>> {
        let context_guard = context.lock().unwrap();
        let serialized = context_guard.serialize();
        HostFunctionResult::Success(serialized)
    }
    
    fn log(
        &self,
        _context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        message: &[u8],
    ) -> HostFunctionResult<()> {
        if let Ok(msg_str) = String::from_utf8(message.to_vec()) {
            println!("{}", msg_str);
        }
        HostFunctionResult::Success(())
    }
    
    fn balance(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        who: &[u8],
        what: &[u8],
    ) -> HostFunctionResult<Vec<u8>> {
        // Implementation would look up balance using the atomic pointer
        // For now, return empty balance
        HostFunctionResult::Success(vec![0; 16]) // 128-bit balance
    }
    
    fn fuel(
        &self,
        _context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u64> {
        // This would need to be integrated with the WASM fuel system
        HostFunctionResult::Success(1000000) // Placeholder
    }
    
    fn height(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u64> {
        let context_guard = context.lock().unwrap();
        HostFunctionResult::Success(context_guard.message.height)
    }
    
    fn sequence(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<u128> {
        let context_guard = context.lock().unwrap();
        // Implementation would get sequence from atomic pointer
        HostFunctionResult::Success(1) // Placeholder
    }
    
    fn request_transaction(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32> {
        let context_guard = context.lock().unwrap();
        // Serialize transaction and return size
        // This would use consensus_encode like in the original
        HostFunctionResult::Success(1000) // Placeholder
    }
    
    fn load_transaction(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>> {
        let context_guard = context.lock().unwrap();
        // Serialize and return transaction data
        HostFunctionResult::Success(vec![]) // Placeholder
    }
    
    fn request_block(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<i32> {
        let context_guard = context.lock().unwrap();
        // Serialize block and return size
        HostFunctionResult::Success(10000) // Placeholder
    }
    
    fn load_block(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>> {
        let context_guard = context.lock().unwrap();
        // Serialize and return block data
        HostFunctionResult::Success(vec![]) // Placeholder
    }
    
    fn returndatacopy(
        &self,
        context: &Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
    ) -> HostFunctionResult<Vec<u8>> {
        let context_guard = context.lock().unwrap();
        HostFunctionResult::Success(context_guard.returndata.clone())
    }
}

/// WASM linker that uses generic host functions
pub struct GenericWasmLinker<KV: KeyValuePointer + Clone, HF: GenericHostFunctions<KV>> {
    host_functions: HF,
    _phantom: std::marker::PhantomData<KV>,
}

impl<KV: KeyValuePointer + Clone, HF: GenericHostFunctions<KV>> GenericWasmLinker<KV, HF> {
    pub fn new(host_functions: HF) -> Self {
        Self {
            host_functions,
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Add host functions to a WASM linker
    pub fn add_to_linker(
        &self,
        linker: &mut Linker<Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>>,
    ) -> Result<()> {
        // Add basic functions
        linker.func_wrap("env", "abort", |_: Caller<_>| {
            // Handle abort
        })?;
        
        // Add storage functions
        linker.func_wrap("env", "__request_storage",
            |caller: Caller<_>, key_ptr: i32| -> i32 {
                // Implementation would read key from WASM memory and call host function
                // For now, return placeholder
                1024
            }
        )?;
        
        linker.func_wrap("env", "__load_storage",
            |caller: Caller<_>, key_ptr: i32, value_ptr: i32| -> i32 {
                // Implementation would read key, call host function, write result
                0
            }
        )?;
        
        // Add other host functions...
        // This would include all the functions from the original host_functions.rs
        // but using the generic trait instead of direct implementation
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::tests::MockKV;
    
    #[test]
    fn test_cpu_host_functions() {
        let host_functions = CpuHostFunctions;
        
        // Test that we can create CPU host functions
        // More comprehensive tests would require setting up a full context
    }
    
    #[test]
    fn test_generic_linker() {
        let host_functions = CpuHostFunctions;
        let _linker = GenericWasmLinker::<MockKV, _>::new(host_functions);
        
        // Test that we can create the generic linker
    }
}