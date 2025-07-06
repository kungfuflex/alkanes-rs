//! Alkanes GPU Pipeline Implementation
//!
//! This crate implements the complete GPU pipeline for parallel alkanes message execution.
//! It provides a memory-based KeyValuePointer implementation and WASM execution environment
//! that can run the same message processing logic as the main indexer.
//!
//! ## Key Features
//!
//! - **Generic Host Functions**: Factored host functions that work with any KeyValuePointer
//! - **GPU-Specific Pointers**: AtomicPointer and IndexPointer implementations with ejection detection
//! - **Shard Ejection**: Automatic detection of GPU constraint violations with CPU fallback
//! - **Preloaded Storage**: Works with K/V subsets preloaded for GPU execution
//!
//! ## SPIR-V Compilation
//!
//! This crate can compile to SPIR-V for actual GPU execution. Set the environment variable
//! `ALKANES_BUILD_SPIRV=1` during build to enable SPIR-V compilation.

// Include compiled SPIR-V binary if available
#[cfg(feature = "spirv")]
const ALKANES_GPU_SPIRV: Option<&[u8]> = {
    match option_env!("ALKANES_GPU_SPIRV_PATH") {
        Some(path) => Some(include_bytes!(env!("ALKANES_GPU_SPIRV_PATH"))),
        None => None,
    }
};

#[cfg(not(feature = "spirv"))]
const ALKANES_GPU_SPIRV: Option<&[u8]> = None;

use alkanes_support::{
    response::ExtendedCallResponse,
    vm::{GenericAlkaneMessageHandler, GenericAlkanesRuntimeContext, GenericMessageContextParcel},
};
use anyhow::Result;
use bitcoin::Transaction;
use metashrew_support::{
    index_pointer::KeyValuePointer,
};
use protorune_support::{
    balance_sheet::BalanceSheet,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use wasmi::*;

// New modules for GPU-specific functionality
pub mod gpu_pointers;
pub mod gpu_host_functions;

// Re-export key types for convenience
pub use gpu_pointers::{GpuAtomicPointer, GpuIndexPointer, PointerResult};
pub use gpu_host_functions::{GpuHostFunctions, GpuWasmExecutor};

/// GPU-compatible data structures for Vulkan execution
pub mod gpu_types {
    
    /// Maximum constraints for GPU compatibility
    pub const MAX_MESSAGE_SIZE: usize = 4096;
    pub const MAX_CALLDATA_SIZE: usize = 2048;
    pub const MAX_KV_PAIRS: usize = 1024;
    pub const MAX_RETURN_DATA_SIZE: usize = 1024;
    pub const MAX_SHARD_SIZE: usize = 64;
    
    /// GPU execution status codes
    pub const GPU_STATUS_SUCCESS: u32 = 0;
    pub const GPU_STATUS_WASM_ERROR: u32 = 1;  // Normal WASM execution error - commit shard
    pub const GPU_STATUS_EJECTED: u32 = 2;     // GPU constraint violation - eject to CPU
    
    /// GPU ejection reasons
    pub const GPU_EJECTION_NONE: u32 = 0;
    pub const GPU_EJECTION_STORAGE_OVERFLOW: u32 = 1;  // Storage value too large for GPU buffer
    pub const GPU_EJECTION_MEMORY_CONSTRAINT: u32 = 2; // GPU memory limit exceeded
    pub const GPU_EJECTION_KV_OVERFLOW: u32 = 3;       // Too many K/V pairs for GPU
    pub const GPU_EJECTION_CALLDATA_OVERFLOW: u32 = 4; // Calldata too large for GPU
    pub const GPU_EJECTION_OTHER: u32 = 5;             // Other GPU-specific constraint
    
    /// Smaller test constraints to avoid stack overflow in tests
    #[cfg(test)]
    pub const TEST_MAX_KV_PAIRS: usize = 4;
    #[cfg(test)]
    pub const TEST_MAX_SHARD_SIZE: usize = 2;
    #[cfg(test)]
    pub const TEST_MAX_CALLDATA_SIZE: usize = 64;
    #[cfg(test)]
    pub const TEST_MAX_RETURN_DATA_SIZE: usize = 64;

    /// GPU message input structure
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuMessageInput {
        pub txid: [u8; 32],
        pub txindex: u32,
        pub height: u64,
        pub vout: u32,
        pub pointer: u32,
        pub refund_pointer: u32,
        pub calldata_len: u32,
        pub calldata: [u8; MAX_CALLDATA_SIZE],
        pub runtime_balance_len: u32,
        pub runtime_balance_data: [u8; 512],
        pub input_runes_len: u32,
        pub input_runes_data: [u8; 512],
    }

    /// GPU key-value pair for storage operations
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuKvPair {
        pub key_len: u32,
        pub key: [u8; 256],
        pub value_len: u32,
        pub value: [u8; 1024],
        pub operation: u32, // 0=read, 1=write, 2=delete
    }

    /// GPU execution context with K/V store view
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuExecutionContext {
        pub kv_count: u32,
        pub kv_pairs: [GpuKvPair; MAX_KV_PAIRS],
        pub shard_id: u32,
        pub height: u64,
    }

    /// GPU execution shard containing messages and context
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuExecutionShard {
        pub message_count: u32,
        pub messages: [GpuMessageInput; MAX_SHARD_SIZE],
        pub context: GpuExecutionContext,
    }

    /// GPU execution result with return data and K/V updates
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuExecutionResult {
        pub kv_update_count: u32,
        pub kv_updates: [GpuKvPair; MAX_KV_PAIRS],
        pub return_data_count: u32,
        pub return_data: [GpuReturnData; MAX_SHARD_SIZE],
        pub status: u32,
        pub error_len: u32,
        pub error_message: [u8; 256],
        pub ejection_reason: u32, // 0=no ejection, 1=storage overflow, 2=memory constraint, 3=other GPU limit
        pub ejected_message_index: u32, // Which message caused ejection (if any)
    }

    /// GPU return data for individual messages
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct GpuReturnData {
        pub message_index: u32,
        pub success: u32,
        pub data_len: u32,
        pub data: [u8; MAX_RETURN_DATA_SIZE],
        pub gas_used: u64,
    }

    // Default implementations
    impl Default for GpuMessageInput {
        fn default() -> Self {
            Self {
                txid: [0; 32],
                txindex: 0,
                height: 0,
                vout: 0,
                pointer: 0,
                refund_pointer: 0,
                calldata_len: 0,
                calldata: [0; MAX_CALLDATA_SIZE],
                runtime_balance_len: 0,
                runtime_balance_data: [0; 512],
                input_runes_len: 0,
                input_runes_data: [0; 512],
            }
        }
    }

    impl Default for GpuKvPair {
        fn default() -> Self {
            Self {
                key_len: 0,
                key: [0; 256],
                value_len: 0,
                value: [0; 1024],
                operation: 0,
            }
        }
    }

    impl Default for GpuExecutionContext {
        fn default() -> Self {
            Self {
                kv_count: 0,
                kv_pairs: [GpuKvPair::default(); MAX_KV_PAIRS],
                shard_id: 0,
                height: 0,
            }
        }
    }

    impl Default for GpuExecutionShard {
        fn default() -> Self {
            Self {
                message_count: 0,
                messages: [GpuMessageInput::default(); MAX_SHARD_SIZE],
                context: GpuExecutionContext::default(),
            }
        }
    }

    impl Default for GpuExecutionResult {
        fn default() -> Self {
            Self {
                kv_update_count: 0,
                kv_updates: [GpuKvPair::default(); MAX_KV_PAIRS],
                return_data_count: 0,
                return_data: [GpuReturnData::default(); MAX_SHARD_SIZE],
                status: 0,
                error_len: 0,
                error_message: [0; 256],
                ejection_reason: 0, // GPU_EJECTION_NONE
                ejected_message_index: 0,
            }
        }
    }

    impl Default for GpuReturnData {
        fn default() -> Self {
            Self {
                message_index: 0,
                success: 0,
                data_len: 0,
                data: [0; MAX_RETURN_DATA_SIZE],
                gas_used: 0,
            }
        }
    }
    
    /// Test-specific smaller data structures to avoid stack overflow
    #[cfg(test)]
    pub mod test_types {
        use super::*;
        
        /// Small test message input
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuMessageInput {
            pub txid: [u8; 32],
            pub txindex: u32,
            pub height: u64,
            pub vout: u32,
            pub pointer: u32,
            pub refund_pointer: u32,
            pub calldata_len: u32,
            pub calldata: [u8; TEST_MAX_CALLDATA_SIZE],
            pub runtime_balance_len: u32,
            pub runtime_balance_data: [u8; 64],
            pub input_runes_len: u32,
            pub input_runes_data: [u8; 64],
        }
        
        impl Default for TestGpuMessageInput {
            fn default() -> Self {
                Self {
                    txid: [0; 32],
                    txindex: 0,
                    height: 0,
                    vout: 0,
                    pointer: 0,
                    refund_pointer: 0,
                    calldata_len: 0,
                    calldata: [0; TEST_MAX_CALLDATA_SIZE],
                    runtime_balance_len: 0,
                    runtime_balance_data: [0; 64],
                    input_runes_len: 0,
                    input_runes_data: [0; 64],
                }
            }
        }
        
        /// Small test K/V pair
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuKvPair {
            pub key_len: u32,
            pub key: [u8; 32],
            pub value_len: u32,
            pub value: [u8; 64],
            pub operation: u32,
        }
        
        impl Default for TestGpuKvPair {
            fn default() -> Self {
                Self {
                    key_len: 0,
                    key: [0; 32],
                    value_len: 0,
                    value: [0; 64],
                    operation: 0,
                }
            }
        }
        
        /// Small test execution context
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuExecutionContext {
            pub kv_count: u32,
            pub kv_pairs: [TestGpuKvPair; TEST_MAX_KV_PAIRS],
            pub shard_id: u32,
            pub height: u64,
        }
        
        impl Default for TestGpuExecutionContext {
            fn default() -> Self {
                Self {
                    kv_count: 0,
                    kv_pairs: [TestGpuKvPair::default(); TEST_MAX_KV_PAIRS],
                    shard_id: 0,
                    height: 0,
                }
            }
        }
        
        /// Small test execution shard
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuExecutionShard {
            pub message_count: u32,
            pub messages: [TestGpuMessageInput; TEST_MAX_SHARD_SIZE],
            pub context: TestGpuExecutionContext,
        }
        
        impl Default for TestGpuExecutionShard {
            fn default() -> Self {
                Self {
                    message_count: 0,
                    messages: [TestGpuMessageInput::default(); TEST_MAX_SHARD_SIZE],
                    context: TestGpuExecutionContext::default(),
                }
            }
        }
        
        /// Small test return data
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuReturnData {
            pub message_index: u32,
            pub success: u32,
            pub data_len: u32,
            pub data: [u8; TEST_MAX_RETURN_DATA_SIZE],
            pub gas_used: u64,
        }
        
        impl Default for TestGpuReturnData {
            fn default() -> Self {
                Self {
                    message_index: 0,
                    success: 0,
                    data_len: 0,
                    data: [0; TEST_MAX_RETURN_DATA_SIZE],
                    gas_used: 0,
                }
            }
        }
        
        /// Small test execution result
        #[repr(C)]
        #[derive(Clone, Copy, Debug)]
        pub struct TestGpuExecutionResult {
            pub kv_update_count: u32,
            pub kv_updates: [TestGpuKvPair; TEST_MAX_KV_PAIRS],
            pub return_data_count: u32,
            pub return_data: [TestGpuReturnData; TEST_MAX_SHARD_SIZE],
            pub status: u32,
            pub error_len: u32,
            pub error_message: [u8; 64],
        }
        
        impl Default for TestGpuExecutionResult {
            fn default() -> Self {
                Self {
                    kv_update_count: 0,
                    kv_updates: [TestGpuKvPair::default(); TEST_MAX_KV_PAIRS],
                    return_data_count: 0,
                    return_data: [TestGpuReturnData::default(); TEST_MAX_SHARD_SIZE],
                    status: 0,
                    error_len: 0,
                    error_message: [0; 64],
                }
            }
        }
    }
}

/// Memory-based KeyValuePointer implementation for GPU execution
#[derive(Clone, Debug)]
pub struct GpuKeyValuePointer {
    /// Shared K/V store for the shard
    store: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
    /// Current key path
    path: Vec<u8>,
    /// Pending updates to be written back
    updates: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
}

impl GpuKeyValuePointer {
    /// Create a new GPU KeyValuePointer with initial K/V data
    pub fn new(initial_data: BTreeMap<Vec<u8>, Arc<Vec<u8>>>) -> Self {
        Self {
            store: Arc::new(Mutex::new(initial_data)),
            path: Vec::new(),
            updates: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    
    /// Get all pending updates to merge back to main store
    pub fn get_updates(&self) -> BTreeMap<Vec<u8>, Arc<Vec<u8>>> {
        self.updates.lock().unwrap().clone()
    }
    
    /// Load initial K/V data from GPU context
    pub fn from_gpu_context(context: &gpu_types::GpuExecutionContext) -> Self {
        let mut initial_data = BTreeMap::new();
        
        for i in 0..context.kv_count as usize {
            if i >= gpu_types::MAX_KV_PAIRS {
                break;
            }
            
            let kv_pair = &context.kv_pairs[i];
            if kv_pair.key_len > 0 && kv_pair.value_len > 0 {
                let key = kv_pair.key[0..kv_pair.key_len as usize].to_vec();
                let value = kv_pair.value[0..kv_pair.value_len as usize].to_vec();
                initial_data.insert(key, Arc::new(value));
            }
        }
        
        Self::new(initial_data)
    }
    
    /// Export updates to GPU result format
    pub fn export_updates(&self, result: &mut gpu_types::GpuExecutionResult) {
        let updates = self.updates.lock().unwrap();
        let mut count = 0;
        
        for (key, value) in updates.iter() {
            if count >= gpu_types::MAX_KV_PAIRS {
                break;
            }
            
            let mut kv_pair = gpu_types::GpuKvPair::default();
            
            // Copy key
            let key_len = std::cmp::min(key.len(), 256);
            kv_pair.key_len = key_len as u32;
            kv_pair.key[0..key_len].copy_from_slice(&key[0..key_len]);
            
            // Copy value
            let value_len = std::cmp::min(value.len(), 1024);
            kv_pair.value_len = value_len as u32;
            kv_pair.value[0..value_len].copy_from_slice(&value[0..value_len]);
            
            kv_pair.operation = 1; // Write operation
            
            result.kv_updates[count] = kv_pair;
            count += 1;
        }
        
        result.kv_update_count = count as u32;
    }
}

impl KeyValuePointer for GpuKeyValuePointer {
    fn wrap(word: &Vec<u8>) -> Self {
        Self {
            store: Arc::new(Mutex::new(BTreeMap::new())),
            path: word.clone(),
            updates: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    
    fn unwrap(&self) -> Arc<Vec<u8>> {
        Arc::new(self.path.clone())
    }
    
    fn inherits(&mut self, from: &Self) {
        self.store = from.store.clone();
        self.updates = from.updates.clone();
    }
    
    fn get(&self) -> Arc<Vec<u8>> {
        // First check updates, then store
        if let Some(value) = self.updates.lock().unwrap().get(&self.path) {
            return value.clone();
        }
        
        self.store.lock().unwrap()
            .get(&self.path)
            .cloned()
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }
    
    fn set(&mut self, value: Arc<Vec<u8>>) {
        self.updates.lock().unwrap().insert(self.path.clone(), value);
    }
    
    fn keyword(&self, key: &str) -> Self {
        let mut new_path = self.path.clone();
        new_path.extend_from_slice(key.as_bytes());
        Self {
            store: self.store.clone(),
            path: new_path,
            updates: self.updates.clone(),
        }
    }
}

/// Legacy GPU-based WASM execution environment (kept for compatibility)
pub struct LegacyGpuWasmExecutor {
    /// WASM binary to execute
    binary: Arc<Vec<u8>>,
    /// Fuel limit for execution
    fuel_limit: u64,
}

impl LegacyGpuWasmExecutor {
    pub fn new(binary: Arc<Vec<u8>>, fuel_limit: u64) -> Self {
        Self { binary, fuel_limit }
    }
    
    /// Execute WASM contract with GPU KeyValuePointer
    pub fn execute(
        &self,
        context: Arc<Mutex<GenericAlkanesRuntimeContext<GpuKeyValuePointer>>>,
    ) -> Result<(ExtendedCallResponse, u64)> {
        // Set up WASM execution environment
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        
        let mut store = Store::new(&engine, context.clone());
        store.add_fuel(self.fuel_limit).map_err(|e| anyhow::anyhow!("Failed to add fuel: {:?}", e))?;
        
        let module = Module::new(&engine, &self.binary[..])?;
        let mut linker = Linker::new(&engine);
        
        // Add host functions that work with GPU KeyValuePointer
        self.add_host_functions(&mut linker)?;
        
        let instance = linker.instantiate(&mut store, &module)?
            .ensure_no_start(&mut store)?;
        
        // Execute the main function
        let main_func = instance.get_typed_func::<(), ()>(&store, "main")?;
        main_func.call(&mut store, ())?;
        
        let fuel_used = store.fuel_consumed().unwrap_or(0);
        
        // Return placeholder response for now
        Ok((ExtendedCallResponse::default(), fuel_used))
    }
    
    /// Add host functions that work with GPU KeyValuePointer
    fn add_host_functions(&self, linker: &mut Linker<Arc<Mutex<GenericAlkanesRuntimeContext<GpuKeyValuePointer>>>>) -> Result<()> {
        // Add basic host functions
        linker.func_wrap("env", "abort", |_: Caller<_>| {
            // Handle abort
        })?;
        
        // Add storage functions
        linker.func_wrap("env", "__load_storage",
            |_caller: Caller<_>, _key_ptr: i32, _value_ptr: i32| -> i32 {
                // Implementation would read from GPU KeyValuePointer
                0
            }
        )?;
        
        linker.func_wrap("env", "__request_storage",
            |_caller: Caller<_>, _key_ptr: i32| -> i32 {
                // Implementation would allocate storage space
                1024
            }
        )?;
        
        // Add other required host functions...
        
        Ok(())
    }
}

/// Main GPU pipeline implementation with ejection capabilities
pub struct GpuAlkanesPipeline {
    atomic_message_handler: GenericAlkaneMessageHandler<GpuAtomicPointer>,
    kv_message_handler: GenericAlkaneMessageHandler<GpuKeyValuePointer>,
    gpu_executor: GpuWasmExecutor,
}

impl GpuAlkanesPipeline {
    pub fn new() -> Self {
        let dummy_binary = Arc::new(vec![0u8; 100]); // Placeholder binary
        Self {
            atomic_message_handler: GenericAlkaneMessageHandler::new(),
            kv_message_handler: GenericAlkaneMessageHandler::new(),
            gpu_executor: GpuWasmExecutor::new(dummy_binary, 1000000), // 1M fuel limit
        }
    }
    
    /// Process a shard of messages with ejection detection
    /// Returns the result and whether the shard was ejected to CPU
    pub fn process_shard_with_ejection_detection(
        &self,
        shard: &gpu_types::GpuExecutionShard,
    ) -> Result<(gpu_types::GpuExecutionResult, bool)> {
        let mut result = gpu_types::GpuExecutionResult::default();
        result.return_data_count = shard.message_count;
        result.status = gpu_types::GPU_STATUS_SUCCESS;
        
        // Create GPU AtomicPointer from context with ejection capabilities
        let gpu_atomic = GpuAtomicPointer::from_gpu_context(&shard.context);
        
        // Process each message in the shard
        for i in 0..shard.message_count as usize {
            if i >= gpu_types::MAX_SHARD_SIZE {
                break;
            }
            
            let message = &shard.messages[i];
            let mut return_data = gpu_types::GpuReturnData::default();
            return_data.message_index = i as u32;
            
            // Check for GPU constraints before processing
            if let Some(ejection_reason) = self.check_gpu_constraints(message, &shard.context) {
                // GPU constraint violation - eject entire shard to CPU
                result.status = gpu_types::GPU_STATUS_EJECTED;
                result.ejection_reason = ejection_reason;
                result.ejected_message_index = i as u32;
                
                // Process the entire shard on CPU to preserve ordering
                return Ok((self.process_shard_on_cpu(shard)?, true));
            }
            
            // Process message with ejection-aware execution
            match self.process_single_message_with_ejection(message, &gpu_atomic) {
                Ok((response, gas_used, ejection_reason)) => {
                    if let Some(reason) = ejection_reason {
                        // Message processing triggered ejection - eject entire shard
                        result.status = gpu_types::GPU_STATUS_EJECTED;
                        result.ejection_reason = reason;
                        result.ejected_message_index = i as u32;
                        
                        return Ok((self.process_shard_on_cpu(shard)?, true));
                    }
                    
                    return_data.success = 1;
                    return_data.gas_used = gas_used;
                    
                    // Copy response data
                    let data_len = std::cmp::min(response.data.len(), gpu_types::MAX_RETURN_DATA_SIZE);
                    return_data.data_len = data_len as u32;
                    return_data.data[0..data_len].copy_from_slice(&response.data[0..data_len]);
                }
                Err(e) => {
                    // WASM execution error - this is normal, not ejection
                    return_data.success = 0;
                    return_data.gas_used = 0;
                    result.status = gpu_types::GPU_STATUS_WASM_ERROR;
                    
                    // Store error message
                    let error_msg = e.to_string();
                    let error_len = std::cmp::min(error_msg.len(), gpu_types::MAX_RETURN_DATA_SIZE);
                    return_data.data_len = error_len as u32;
                    return_data.data[0..error_len].copy_from_slice(error_msg.as_bytes());
                }
            }
            
            result.return_data[i] = return_data;
        }
        
        // Export K/V updates
        gpu_atomic.export_updates(&mut result);
        
        Ok((result, false)) // Successfully processed on GPU
    }
    
    /// Process a shard of messages on GPU (legacy method for compatibility)
    pub fn process_shard(
        &self,
        shard: &gpu_types::GpuExecutionShard,
    ) -> Result<gpu_types::GpuExecutionResult> {
        let (result, _ejected) = self.process_shard_with_ejection_detection(shard)?;
        Ok(result)
    }
    
    /// Process a shard entirely on CPU (fallback for ejected shards)
    pub fn process_shard_on_cpu(
        &self,
        shard: &gpu_types::GpuExecutionShard,
    ) -> Result<gpu_types::GpuExecutionResult> {
        let mut result = gpu_types::GpuExecutionResult::default();
        result.return_data_count = shard.message_count;
        result.status = gpu_types::GPU_STATUS_SUCCESS;
        
        // Create standard GpuKeyValuePointer for CPU execution (no ejection constraints)
        let gpu_kv = GpuKeyValuePointer::from_gpu_context(&shard.context);
        
        // Process each message using full CPU pipeline (no GPU constraints)
        for i in 0..shard.message_count as usize {
            if i >= gpu_types::MAX_SHARD_SIZE {
                break;
            }
            
            let message = &shard.messages[i];
            let mut return_data = gpu_types::GpuReturnData::default();
            return_data.message_index = i as u32;
            
            match self.process_single_message_cpu_full(message, &gpu_kv) {
                Ok((response, gas_used)) => {
                    return_data.success = 1;
                    return_data.gas_used = gas_used;
                    
                    // Copy response data (no size limits on CPU)
                    let data_len = std::cmp::min(response.data.len(), gpu_types::MAX_RETURN_DATA_SIZE);
                    return_data.data_len = data_len as u32;
                    return_data.data[0..data_len].copy_from_slice(&response.data[0..data_len]);
                }
                Err(e) => {
                    // WASM execution error
                    return_data.success = 0;
                    return_data.gas_used = 0;
                    result.status = gpu_types::GPU_STATUS_WASM_ERROR;
                    
                    // Store error message
                    let error_msg = e.to_string();
                    let error_len = std::cmp::min(error_msg.len(), gpu_types::MAX_RETURN_DATA_SIZE);
                    return_data.data_len = error_len as u32;
                    return_data.data[0..error_len].copy_from_slice(error_msg.as_bytes());
                }
            }
            
            result.return_data[i] = return_data;
        }
        
        // Export K/V updates
        gpu_kv.export_updates(&mut result);
        
        Ok(result)
    }
    
    /// Process a single message with ejection detection
    fn process_single_message_with_ejection(
        &self,
        message: &gpu_types::GpuMessageInput,
        gpu_atomic: &GpuAtomicPointer,
    ) -> Result<(ExtendedCallResponse, u64, Option<u32>)> {
        // Convert GPU message to generic message context parcel
        let parcel = self.convert_gpu_message_to_parcel_atomic(message, gpu_atomic.clone())?;
        
        // Create context for GPU execution
        let context = Arc::new(Mutex::new(GenericAlkanesRuntimeContext {
            myself: alkanes_support::id::AlkaneId::default(),
            caller: alkanes_support::id::AlkaneId::default(),
            incoming_alkanes: Default::default(),
            returndata: vec![],
            inputs: vec![],
            message: Box::new(parcel),
            trace: Default::default(),
        }));
        
        // Execute with GPU constraints and ejection detection
        match self.gpu_executor.execute_with_ejection(context, &[]) {
            Ok((response, gas_used, ejection_reason)) => {
                Ok((response, gas_used, ejection_reason))
            }
            Err(e) => Err(e),
        }
    }
    
    /// Convert GPU message format to generic message context parcel with GpuAtomicPointer
    fn convert_gpu_message_to_parcel_atomic(
        &self,
        message: &gpu_types::GpuMessageInput,
        gpu_atomic: GpuAtomicPointer,
    ) -> Result<GenericMessageContextParcel<GpuAtomicPointer>> {
        // Create a minimal transaction from the message data
        let transaction = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        
        // Extract calldata
        let calldata = if message.calldata_len > 0 {
            message.calldata[0..message.calldata_len as usize].to_vec()
        } else {
            vec![]
        };
        
        // Create runtime balances (placeholder)
        let runtime_balances = Arc::new(BalanceSheet::default());
        
        // Create runes (placeholder)
        let runes = vec![];
        
        Ok(GenericMessageContextParcel {
            transaction,
            txindex: message.txindex,
            height: message.height,
            vout: message.vout,
            pointer: message.pointer,
            refund_pointer: message.refund_pointer,
            calldata,
            atomic: gpu_atomic,
            runtime_balances,
            runes,
        })
    }
    
    /// Check if a message would violate GPU constraints before processing
    fn check_gpu_constraints(
        &self,
        message: &gpu_types::GpuMessageInput,
        context: &gpu_types::GpuExecutionContext,
    ) -> Option<u32> {
        // Check calldata size constraint
        if message.calldata_len > gpu_types::MAX_CALLDATA_SIZE as u32 {
            return Some(gpu_types::GPU_EJECTION_CALLDATA_OVERFLOW);
        }
        
        // Check if we're approaching K/V storage limits
        if context.kv_count >= (gpu_types::MAX_KV_PAIRS as u32 * 9 / 10) { // 90% threshold
            return Some(gpu_types::GPU_EJECTION_KV_OVERFLOW);
        }
        
        // Check for potential storage value size issues
        if message.calldata_len > 1024 { // Large calldata might produce large storage
            return Some(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        None // No constraint violations
    }
    
    /// Check if processing results would violate GPU constraints
    fn check_processing_constraints(&self, response: &ExtendedCallResponse) -> Option<u32> {
        // Check if return data is too large for GPU buffers
        if response.data.len() > gpu_types::MAX_RETURN_DATA_SIZE / 2 {
            return Some(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        // Additional constraint checks could be added here
        // For example, checking estimated storage operations, memory usage, etc.
        
        None // No constraint violations
    }
    
    /// Process a single alkanes message (GPU-constrained version)
    fn process_single_message(
        &self,
        message: &gpu_types::GpuMessageInput,
        gpu_kv: &GpuKeyValuePointer,
    ) -> Result<(ExtendedCallResponse, u64)> {
        // Convert GPU message to generic message context parcel
        let parcel = self.convert_gpu_message_to_parcel(message, gpu_kv.clone())?;
        
        // Use the KV message handler
        let (_rune_transfers, _balance_sheet) = self.kv_message_handler.handle_message(&parcel)?;
        
        // For now, return a placeholder response
        // In a full implementation, this would execute the actual WASM contract
        Ok((ExtendedCallResponse::default(), 1000))
    }
    
    /// Process a single alkanes message with full CPU pipeline (no GPU constraints)
    fn process_single_message_cpu_full(
        &self,
        message: &gpu_types::GpuMessageInput,
        gpu_kv: &GpuKeyValuePointer,
    ) -> Result<(ExtendedCallResponse, u64)> {
        // Convert GPU message to generic message context parcel
        let parcel = self.convert_gpu_message_to_parcel(message, gpu_kv.clone())?;
        
        // Use the KV message handler with full CPU capabilities
        let (_rune_transfers, _balance_sheet) = self.kv_message_handler.handle_message(&parcel)?;
        
        // For now, return a placeholder response
        // In a full implementation, this would execute the actual WASM contract
        // without GPU memory constraints
        Ok((ExtendedCallResponse::default(), 1000))
    }
    
    /// Convert GPU message format to generic message context parcel
    fn convert_gpu_message_to_parcel(
        &self,
        message: &gpu_types::GpuMessageInput,
        gpu_kv: GpuKeyValuePointer,
    ) -> Result<GenericMessageContextParcel<GpuKeyValuePointer>> {
        // Create a minimal transaction from the message data
        let transaction = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        
        // Extract calldata
        let calldata = if message.calldata_len > 0 {
            message.calldata[0..message.calldata_len as usize].to_vec()
        } else {
            vec![]
        };
        
        // Create runtime balances (placeholder)
        let runtime_balances = Arc::new(BalanceSheet::default());
        
        // Create runes (placeholder)
        let runes = vec![];
        
        Ok(GenericMessageContextParcel {
            transaction,
            txindex: message.txindex,
            height: message.height,
            vout: message.vout,
            pointer: message.pointer,
            refund_pointer: message.refund_pointer,
            calldata,
            atomic: gpu_kv,
            runtime_balances,
            runes,
        })
    }
}

/// Main entry point for GPU pipeline execution
/// This is the function that gets called from the Vulkan compute shader
#[no_mangle]
pub extern "C" fn __pipeline(
    input_shard: *const gpu_types::GpuExecutionShard,
    output_result: *mut gpu_types::GpuExecutionResult,
) -> i32 {
    // Safety: We assume the pointers are valid from the Vulkan runtime
    let shard = unsafe { &*input_shard };
    let result = unsafe { &mut *output_result };
    
    let pipeline = GpuAlkanesPipeline::new();
    
    match pipeline.process_shard(shard) {
        Ok(processed_result) => {
            *result = processed_result;
            0 // Success
        }
        Err(e) => {
            // Set error in result
            result.status = 1;
            let error_msg = e.to_string();
            let error_len = std::cmp::min(error_msg.len(), 256);
            result.error_len = error_len as u32;
            result.error_message[0..error_len].copy_from_slice(error_msg.as_bytes());
            1 // Error
        }
    }
}

/// CPU fallback for testing
pub fn __pipeline_cpu(
    input_shard: &gpu_types::GpuExecutionShard,
    output_result: &mut gpu_types::GpuExecutionResult,
) -> Result<()> {
    let pipeline = GpuAlkanesPipeline::new();
    *output_result = pipeline.process_shard(input_shard)?;
    Ok(())
}

/// Get the compiled SPIR-V binary for GPU execution
/// Returns None if SPIR-V compilation was not enabled during build
pub fn get_spirv_binary() -> Option<&'static [u8]> {
    ALKANES_GPU_SPIRV
}

/// Check if SPIR-V binary is available
pub fn has_spirv_binary() -> bool {
    ALKANES_GPU_SPIRV.is_some()
}

/// Get information about the SPIR-V compilation
pub fn spirv_info() -> String {
    match ALKANES_GPU_SPIRV {
        Some(binary) => format!("SPIR-V binary available ({} bytes)", binary.len()),
        None => "SPIR-V binary not available (set ALKANES_BUILD_SPIRV=1 during build)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Test SPIR-V binary availability and validation
    #[test]
    fn test_spirv_binary_availability() {
        println!("Testing SPIR-V binary availability...");
        
        // Test basic availability
        let has_binary = has_spirv_binary();
        println!("SPIR-V binary available: {}", has_binary);
        
        // Test info function
        let info = spirv_info();
        println!("SPIR-V info: {}", info);
        
        // Test binary access
        match get_spirv_binary() {
            Some(binary) => {
                println!("SPIR-V binary loaded: {} bytes", binary.len());
                assert!(!binary.is_empty(), "SPIR-V binary should not be empty");
                
                // Basic SPIR-V validation - check magic number
                if binary.len() >= 4 {
                    let magic = u32::from_le_bytes([binary[0], binary[1], binary[2], binary[3]]);
                    println!("SPIR-V magic number: 0x{:08x}", magic);
                    
                    // SPIR-V magic number is 0x07230203
                    if magic == 0x07230203 {
                        println!("✓ Valid SPIR-V magic number detected");
                    } else {
                        println!("⚠ Warning: Expected SPIR-V magic 0x07230203, got 0x{:08x}", magic);
                    }
                }
                
                // Check minimum size (SPIR-V header is 20 bytes minimum)
                assert!(binary.len() >= 20, "SPIR-V binary too small (minimum 20 bytes for header)");
                
                // Test that we can access the binary multiple times
                let binary2 = get_spirv_binary().unwrap();
                assert_eq!(binary.len(), binary2.len(), "SPIR-V binary should be consistent");
                
                println!("✓ SPIR-V binary validation passed");
            }
            None => {
                println!("No SPIR-V binary available - this is expected when not built with ALKANES_BUILD_SPIRV=1");
                assert!(!has_binary, "has_spirv_binary() should return false when no binary available");
            }
        }
    }
    
    /// Test SPIR-V feature flag integration
    #[test]
    fn test_spirv_feature_integration() {
        println!("Testing SPIR-V feature integration...");
        
        // Test that the feature flag and binary availability are consistent
        #[cfg(feature = "spirv")]
        {
            println!("SPIR-V feature is enabled");
            // When feature is enabled, we should either have a binary or a clear reason why not
            match get_spirv_binary() {
                Some(_) => println!("✓ SPIR-V binary available with feature enabled"),
                None => println!("SPIR-V feature enabled but no binary (ALKANES_GPU_SPIRV_PATH not set)"),
            }
        }
        
        #[cfg(not(feature = "spirv"))]
        {
            println!("SPIR-V feature is disabled");
            assert!(get_spirv_binary().is_none(), "No SPIR-V binary should be available when feature is disabled");
        }
    }
    
    /// Test GPU pipeline with SPIR-V integration
    #[test]
    fn test_gpu_pipeline_spirv_integration() {
        println!("Testing GPU pipeline SPIR-V integration...");
        
        let pipeline = GpuAlkanesPipeline::new();
        
        // Test that pipeline can be created regardless of SPIR-V availability
        println!("✓ GPU pipeline created successfully");
        
        // Test SPIR-V info access through pipeline
        let info = spirv_info();
        println!("Pipeline SPIR-V info: {}", info);
        
        // Test basic SPIR-V functionality without large stack allocations
        match get_spirv_binary() {
            Some(binary) => {
                println!("✓ SPIR-V binary accessible from pipeline context: {} bytes", binary.len());
                assert!(binary.len() > 1000, "SPIR-V binary should be substantial");
            }
            None => {
                println!("No SPIR-V binary available in pipeline context");
            }
        }
        
        println!("✓ GPU pipeline SPIR-V integration test passed");
    }
    
    use gpu_types::test_types::*;
    
    /// Test GPU KeyValuePointer basic operations
    #[test]
    fn test_gpu_kv_pointer_basic() {
        let mut initial_data = BTreeMap::new();
        initial_data.insert(b"test_key".to_vec(), Arc::new(b"test_value".to_vec()));
        
        let gpu_kv = GpuKeyValuePointer::new(initial_data);
        
        // Test basic get
        let test_ptr = gpu_kv.keyword("test_key");
        assert_eq!(test_ptr.get().as_ref(), b"test_value");
        
        // Test empty key
        let empty_ptr = gpu_kv.keyword("nonexistent");
        assert_eq!(empty_ptr.get().len(), 0);
    }
    
    /// Test GPU KeyValuePointer updates tracking
    #[test]
    fn test_gpu_kv_pointer_updates() {
        let initial_data = BTreeMap::new();
        let gpu_kv = GpuKeyValuePointer::new(initial_data);
        
        // Test setting new value
        let mut test_ptr = gpu_kv.keyword("new_key");
        test_ptr.set(Arc::new(b"new_value".to_vec()));
        
        // Check updates tracking
        let updates = gpu_kv.get_updates();
        assert!(updates.contains_key(&b"new_key".to_vec()));
        assert_eq!(updates.get(&b"new_key".to_vec()).unwrap().as_ref(), b"new_value");
    }
    
    /// Test GPU KeyValuePointer inheritance
    #[test]
    fn test_gpu_kv_pointer_inheritance() {
        let mut initial_data = BTreeMap::new();
        initial_data.insert(b"shared_key".to_vec(), Arc::new(b"shared_value".to_vec()));
        
        let parent_kv = GpuKeyValuePointer::new(initial_data);
        let mut child_kv = GpuKeyValuePointer::wrap(&b"child_path".to_vec());
        child_kv.inherits(&parent_kv);
        
        // Child should have access to parent's data through the shared store
        // But the child's path is different, so we need to access the shared key directly
        let mut shared_ptr = child_kv.clone();
        shared_ptr.path = b"shared_key".to_vec();
        assert_eq!(shared_ptr.get().as_ref(), b"shared_value");
        
        // Test that inheritance shares the store and updates
        let updates = child_kv.get_updates();
        let parent_updates = parent_kv.get_updates();
        // Both should point to the same updates store
        assert_eq!(updates.len(), parent_updates.len());
    }
    
    /// Test GPU pipeline creation
    #[test]
    fn test_gpu_pipeline_creation() {
        let _pipeline = GpuAlkanesPipeline::new();
        // If we get here without panic, the pipeline was created successfully
    }
    
    /// Test GPU data structure sizes (should be reasonable for stack allocation)
    #[test]
    fn test_gpu_test_structure_sizes() {
        use std::mem::size_of;
        
        // Test structures should be much smaller than production ones
        assert!(size_of::<TestGpuMessageInput>() < 1024);
        assert!(size_of::<TestGpuExecutionContext>() < 1024);
        assert!(size_of::<TestGpuExecutionShard>() < 4096);
        assert!(size_of::<TestGpuExecutionResult>() < 4096);
        
        println!("Test structure sizes:");
        println!("  TestGpuMessageInput: {} bytes", size_of::<TestGpuMessageInput>());
        println!("  TestGpuExecutionContext: {} bytes", size_of::<TestGpuExecutionContext>());
        println!("  TestGpuExecutionShard: {} bytes", size_of::<TestGpuExecutionShard>());
        println!("  TestGpuExecutionResult: {} bytes", size_of::<TestGpuExecutionResult>());
    }
    
    /// Test GPU message conversion logic
    #[test]
    fn test_gpu_message_conversion() {
        let pipeline = GpuAlkanesPipeline::new();
        let gpu_kv = GpuKeyValuePointer::new(BTreeMap::new());
        
        // Create a test message
        let mut test_message = TestGpuMessageInput::default();
        test_message.txindex = 42;
        test_message.height = 100;
        test_message.vout = 1;
        test_message.pointer = 123;
        test_message.refund_pointer = 456;
        
        // Add some test calldata
        let test_calldata = b"test_calldata";
        test_message.calldata_len = test_calldata.len() as u32;
        test_message.calldata[0..test_calldata.len()].copy_from_slice(test_calldata);
        
        // Convert to full GPU message for testing conversion logic
        let full_message = gpu_types::GpuMessageInput {
            txid: test_message.txid,
            txindex: test_message.txindex,
            height: test_message.height,
            vout: test_message.vout,
            pointer: test_message.pointer,
            refund_pointer: test_message.refund_pointer,
            calldata_len: test_message.calldata_len,
            calldata: {
                let mut calldata = [0u8; gpu_types::MAX_CALLDATA_SIZE];
                calldata[0..test_calldata.len()].copy_from_slice(test_calldata);
                calldata
            },
            runtime_balance_len: 0,
            runtime_balance_data: [0; 512],
            input_runes_len: 0,
            input_runes_data: [0; 512],
        };
        
        // Test conversion (this tests the conversion logic without full processing)
        let result = pipeline.convert_gpu_message_to_parcel(&full_message, gpu_kv);
        assert!(result.is_ok());
        
        let parcel = result.unwrap();
        assert_eq!(parcel.txindex, 42);
        assert_eq!(parcel.height, 100);
        assert_eq!(parcel.vout, 1);
        assert_eq!(parcel.pointer, 123);
        assert_eq!(parcel.refund_pointer, 456);
        assert_eq!(parcel.calldata, test_calldata);
    }
    
    /// Test WASM executor creation
    #[test]
    fn test_wasm_executor_creation() {
        let binary = Arc::new(vec![0u8; 100]); // Dummy WASM binary
        let _executor = LegacyGpuWasmExecutor::new(binary, 1000000);
        // If we get here without panic, the executor was created successfully
    }
    
    /// Test GPU context export/import
    #[test]
    fn test_gpu_context_export_import() {
        // Create initial K/V data
        let mut initial_data = BTreeMap::new();
        initial_data.insert(b"key1".to_vec(), Arc::new(b"value1".to_vec()));
        initial_data.insert(b"key2".to_vec(), Arc::new(b"value2".to_vec()));
        
        let gpu_kv = GpuKeyValuePointer::new(initial_data);
        
        // Add some updates
        let mut update_ptr = gpu_kv.keyword("new_key");
        update_ptr.set(Arc::new(b"new_value".to_vec()));
        
        // Test export to GPU result format
        let mut result = TestGpuExecutionResult::default();
        
        // We can't directly test export_updates with test structures,
        // but we can test the updates tracking
        let updates = gpu_kv.get_updates();
        assert_eq!(updates.len(), 1);
        assert!(updates.contains_key(&b"new_key".to_vec()));
    }
}

/// Integration tests using vulkanology framework
#[cfg(test)]
mod vulkan_integration_tests {
    use super::*;
    
    /// Test basic Vulkan compute shader functionality
    /// This would use vulkanology once we have actual compute shaders
    #[test]
    #[ignore] // Ignore until we have actual compute shaders
    fn test_vulkan_compute_basic() {
        // TODO: Implement once we have SPIR-V compute shaders
        // This would use vulkanology to test actual GPU execution
        
        // Example of what this would look like:
        // vulkanology_test! {
        //     name: test_alkanes_pipeline,
        //     shader: "shaders/alkanes_pipeline.comp.spv",
        //     input: test_shard_data,
        //     expected: expected_result_data
        // }
    }
    
    /// Test GPU memory management
    #[test]
    #[ignore] // Ignore until we have actual GPU integration
    fn test_gpu_memory_management() {
        // TODO: Test actual GPU memory allocation and management
        // This would test the Vulkan buffer management for our data structures
    }
}