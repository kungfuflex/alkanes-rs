//! GPU-specific pointer implementations with ejection capabilities
//! 
//! This module provides KeyValuePointer implementations that work with preloaded
//! K/V subsets and can detect when operations go outside the allowed storage slots,
//! triggering shard ejection to CPU execution.

use crate::gpu_types;
use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Result of a pointer operation that may trigger ejection
#[derive(Debug, Clone)]
pub enum PointerResult<T> {
    /// Operation completed successfully
    Success(T),
    /// Operation accessed storage outside preloaded subset - eject shard
    Ejected(u32), // ejection reason code
}

impl<T> PointerResult<T> {
    pub fn is_ejected(&self) -> bool {
        matches!(self, PointerResult::Ejected(_))
    }
    
    pub fn ejection_reason(&self) -> Option<u32> {
        match self {
            PointerResult::Ejected(reason) => Some(*reason),
            _ => None,
        }
    }
    
    pub fn unwrap_or_eject(self, default_ejection: u32) -> Result<T, u32> {
        match self {
            PointerResult::Success(value) => Ok(value),
            PointerResult::Ejected(reason) => Err(reason),
        }
    }
}

/// GPU AtomicPointer that works with preloaded K/V subsets and detects ejection conditions
#[derive(Clone, Debug)]
pub struct GpuAtomicPointer {
    /// Current key path
    path: Vec<u8>,
    /// Preloaded input K/V store (read-only)
    input_store: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
    /// Output buffer for writes (acts like cache)
    output_buffer: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
    /// Checkpoint stack for transaction semantics
    checkpoint_stack: Arc<Mutex<Vec<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>>,
    /// Set of allowed storage keys (preloaded subset)
    allowed_keys: Arc<Mutex<BTreeMap<Vec<u8>, bool>>>,
}

impl GpuAtomicPointer {
    /// Create a new GPU AtomicPointer with preloaded K/V data
    pub fn new(
        initial_data: BTreeMap<Vec<u8>, Arc<Vec<u8>>>,
        allowed_keys: BTreeMap<Vec<u8>, bool>,
    ) -> Self {
        Self {
            path: Vec::new(),
            input_store: Arc::new(Mutex::new(initial_data)),
            output_buffer: Arc::new(Mutex::new(BTreeMap::new())),
            checkpoint_stack: Arc::new(Mutex::new(vec![BTreeMap::new()])),
            allowed_keys: Arc::new(Mutex::new(allowed_keys)),
        }
    }
    
    /// Create from GPU execution context
    pub fn from_gpu_context(context: &gpu_types::GpuExecutionContext) -> Self {
        let mut initial_data = BTreeMap::new();
        let mut allowed_keys = BTreeMap::new();
        
        for i in 0..context.kv_count as usize {
            if i >= gpu_types::MAX_KV_PAIRS {
                break;
            }
            
            let kv_pair = &context.kv_pairs[i];
            if kv_pair.key_len > 0 {
                let key = kv_pair.key[0..kv_pair.key_len as usize].to_vec();
                let value = if kv_pair.value_len > 0 {
                    kv_pair.value[0..kv_pair.value_len as usize].to_vec()
                } else {
                    vec![]
                };
                
                initial_data.insert(key.clone(), Arc::new(value));
                allowed_keys.insert(key, true);
            }
        }
        
        Self::new(initial_data, allowed_keys)
    }
    
    /// Check if a key is in the allowed preloaded subset
    fn is_key_allowed(&self, key: &[u8]) -> bool {
        self.allowed_keys.lock().unwrap().contains_key(key)
    }
    
    /// Get value with ejection detection
    pub fn get_with_ejection(&self) -> PointerResult<Arc<Vec<u8>>> {
        let key = &self.path;
        
        // Check if key is in allowed subset
        if !self.is_key_allowed(key) {
            return PointerResult::Ejected(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        // First check output buffer (writes)
        if let Some(value) = self.output_buffer.lock().unwrap().get(key) {
            return PointerResult::Success(value.clone());
        }
        
        // Then check checkpoint stack
        let checkpoint_stack = self.checkpoint_stack.lock().unwrap();
        for checkpoint in checkpoint_stack.iter().rev() {
            if let Some(value) = checkpoint.get(key) {
                return PointerResult::Success(value.clone());
            }
        }
        
        // Finally check input store
        if let Some(value) = self.input_store.lock().unwrap().get(key) {
            PointerResult::Success(value.clone())
        } else {
            PointerResult::Success(Arc::new(Vec::new()))
        }
    }
    
    /// Set value with ejection detection
    pub fn set_with_ejection(&mut self, value: Arc<Vec<u8>>) -> PointerResult<()> {
        let key = &self.path;
        
        // Check if key is in allowed subset
        if !self.is_key_allowed(key) {
            return PointerResult::Ejected(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        // Check if value size exceeds GPU constraints
        if value.len() > 1024 { // MAX_STORAGE_VALUE_SIZE
            return PointerResult::Ejected(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        // Write to current checkpoint
        let mut checkpoint_stack = self.checkpoint_stack.lock().unwrap();
        if let Some(current_checkpoint) = checkpoint_stack.last_mut() {
            current_checkpoint.insert(key.clone(), value);
        }
        
        PointerResult::Success(())
    }
    
    /// Create a checkpoint (transaction boundary)
    pub fn checkpoint(&mut self) {
        self.checkpoint_stack.lock().unwrap().push(BTreeMap::new());
    }
    
    /// Commit current checkpoint
    pub fn commit(&mut self) {
        let mut checkpoint_stack = self.checkpoint_stack.lock().unwrap();
        if checkpoint_stack.len() > 1 {
            let current = checkpoint_stack.pop().unwrap();
            // Merge into previous checkpoint
            if let Some(previous) = checkpoint_stack.last_mut() {
                for (k, v) in current {
                    previous.insert(k, v);
                }
            }
        } else if checkpoint_stack.len() == 1 {
            // Commit to output buffer
            let current = checkpoint_stack.last().unwrap();
            let mut output_buffer = self.output_buffer.lock().unwrap();
            for (k, v) in current {
                output_buffer.insert(k.clone(), v.clone());
            }
        }
    }
    
    /// Rollback current checkpoint
    pub fn rollback(&mut self) {
        let mut checkpoint_stack = self.checkpoint_stack.lock().unwrap();
        if checkpoint_stack.len() > 1 {
            checkpoint_stack.pop();
        }
    }
    
    /// Get checkpoint depth
    pub fn checkpoint_depth(&self) -> usize {
        self.checkpoint_stack.lock().unwrap().len()
    }
    
    /// Export all updates to GPU result format
    pub fn export_updates(&self, result: &mut gpu_types::GpuExecutionResult) {
        let output_buffer = self.output_buffer.lock().unwrap();
        let mut count = 0;
        
        for (key, value) in output_buffer.iter() {
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

impl KeyValuePointer for GpuAtomicPointer {
    fn wrap(word: &Vec<u8>) -> Self {
        Self {
            path: word.clone(),
            input_store: Arc::new(Mutex::new(BTreeMap::new())),
            output_buffer: Arc::new(Mutex::new(BTreeMap::new())),
            checkpoint_stack: Arc::new(Mutex::new(vec![BTreeMap::new()])),
            allowed_keys: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    
    fn unwrap(&self) -> Arc<Vec<u8>> {
        Arc::new(self.path.clone())
    }
    
    fn inherits(&mut self, from: &Self) {
        self.input_store = from.input_store.clone();
        self.output_buffer = from.output_buffer.clone();
        self.checkpoint_stack = from.checkpoint_stack.clone();
        self.allowed_keys = from.allowed_keys.clone();
    }
    
    fn get(&self) -> Arc<Vec<u8>> {
        match self.get_with_ejection() {
            PointerResult::Success(value) => value,
            PointerResult::Ejected(_) => {
                // In the standard KeyValuePointer interface, we can't return ejection
                // The ejection detection should be handled at a higher level
                Arc::new(Vec::new())
            }
        }
    }
    
    fn set(&mut self, value: Arc<Vec<u8>>) {
        match self.set_with_ejection(value) {
            PointerResult::Success(_) => {},
            PointerResult::Ejected(_) => {
                // In the standard KeyValuePointer interface, we can't signal ejection
                // The ejection detection should be handled at a higher level
            }
        }
    }
    
    fn keyword(&self, key: &str) -> Self {
        let mut new_path = self.path.clone();
        new_path.extend_from_slice(key.as_bytes());
        Self {
            path: new_path,
            input_store: self.input_store.clone(),
            output_buffer: self.output_buffer.clone(),
            checkpoint_stack: self.checkpoint_stack.clone(),
            allowed_keys: self.allowed_keys.clone(),
        }
    }
}

/// GPU IndexPointer that works with preloaded subsets (simpler, read-only version)
#[derive(Clone, Debug)]
pub struct GpuIndexPointer {
    /// Current key path
    path: Vec<u8>,
    /// Preloaded K/V store
    store: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
    /// Set of allowed storage keys
    allowed_keys: Arc<Mutex<BTreeMap<Vec<u8>, bool>>>,
}

impl GpuIndexPointer {
    pub fn new(
        initial_data: BTreeMap<Vec<u8>, Arc<Vec<u8>>>,
        allowed_keys: BTreeMap<Vec<u8>, bool>,
    ) -> Self {
        Self {
            path: Vec::new(),
            store: Arc::new(Mutex::new(initial_data)),
            allowed_keys: Arc::new(Mutex::new(allowed_keys)),
        }
    }
    
    /// Check if a key is in the allowed preloaded subset
    fn is_key_allowed(&self, key: &[u8]) -> bool {
        self.allowed_keys.lock().unwrap().contains_key(key)
    }
    
    /// Get value with ejection detection
    pub fn get_with_ejection(&self) -> PointerResult<Arc<Vec<u8>>> {
        let key = &self.path;
        
        if !self.is_key_allowed(key) {
            return PointerResult::Ejected(gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
        }
        
        if let Some(value) = self.store.lock().unwrap().get(key) {
            PointerResult::Success(value.clone())
        } else {
            PointerResult::Success(Arc::new(Vec::new()))
        }
    }
}

impl KeyValuePointer for GpuIndexPointer {
    fn wrap(word: &Vec<u8>) -> Self {
        Self {
            path: word.clone(),
            store: Arc::new(Mutex::new(BTreeMap::new())),
            allowed_keys: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    
    fn unwrap(&self) -> Arc<Vec<u8>> {
        Arc::new(self.path.clone())
    }
    
    fn inherits(&mut self, from: &Self) {
        self.store = from.store.clone();
        self.allowed_keys = from.allowed_keys.clone();
    }
    
    fn get(&self) -> Arc<Vec<u8>> {
        match self.get_with_ejection() {
            PointerResult::Success(value) => value,
            PointerResult::Ejected(_) => Arc::new(Vec::new()),
        }
    }
    
    fn set(&mut self, _value: Arc<Vec<u8>>) {
        // GPU IndexPointer is read-only in this implementation
        // Writes should go through GpuAtomicPointer
    }
    
    fn keyword(&self, key: &str) -> Self {
        let mut new_path = self.path.clone();
        new_path.extend_from_slice(key.as_bytes());
        Self {
            path: new_path,
            store: self.store.clone(),
            allowed_keys: self.allowed_keys.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gpu_atomic_pointer_allowed_access() {
        let mut initial_data = BTreeMap::new();
        let mut allowed_keys = BTreeMap::new();
        
        let key = b"test_key".to_vec();
        let value = b"test_value".to_vec();
        
        initial_data.insert(key.clone(), Arc::new(value.clone()));
        allowed_keys.insert(key.clone(), true);
        
        let gpu_ptr = GpuAtomicPointer::new(initial_data, allowed_keys);
        let test_ptr = gpu_ptr.keyword("test_key");
        
        match test_ptr.get_with_ejection() {
            PointerResult::Success(result) => {
                assert_eq!(result.as_ref(), &value);
            }
            PointerResult::Ejected(_) => {
                panic!("Should not eject for allowed key");
            }
        }
    }
    
    #[test]
    fn test_gpu_atomic_pointer_ejection() {
        let initial_data = BTreeMap::new();
        let allowed_keys = BTreeMap::new(); // Empty - no keys allowed
        
        let gpu_ptr = GpuAtomicPointer::new(initial_data, allowed_keys);
        let test_ptr = gpu_ptr.keyword("forbidden_key");
        
        match test_ptr.get_with_ejection() {
            PointerResult::Success(_) => {
                panic!("Should eject for forbidden key");
            }
            PointerResult::Ejected(reason) => {
                assert_eq!(reason, gpu_types::GPU_EJECTION_STORAGE_OVERFLOW);
            }
        }
    }
    
    #[test]
    fn test_gpu_atomic_pointer_checkpoint() {
        let mut initial_data = BTreeMap::new();
        let mut allowed_keys = BTreeMap::new();
        
        let key = b"test_key".to_vec();
        allowed_keys.insert(key.clone(), true);
        
        let mut gpu_ptr = GpuAtomicPointer::new(initial_data, allowed_keys);
        
        // Create checkpoint
        gpu_ptr.checkpoint();
        
        // Set value
        let mut test_ptr = gpu_ptr.keyword("test_key");
        let result = test_ptr.set_with_ejection(Arc::new(b"test_value".to_vec()));
        assert!(matches!(result, PointerResult::Success(_)));
        
        // Commit
        gpu_ptr.commit();
        
        // Verify value is accessible
        match test_ptr.get_with_ejection() {
            PointerResult::Success(value) => {
                assert_eq!(value.as_ref(), b"test_value");
            }
            PointerResult::Ejected(_) => {
                panic!("Should not eject after commit");
            }
        }
    }
}