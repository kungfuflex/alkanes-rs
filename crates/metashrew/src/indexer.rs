//! Indexer functionality for Metashrew.
//!
//! This module provides the core functionality for building Metashrew indexer programs.

use anyhow::Result;
use std::collections::HashMap;
use std::any::Any;
use crate::view::{ViewFunction, ProtoViewFunction};

/// A trait for types that can be indexed by Metashrew
pub trait Indexer {
    /// Process a block and update the state
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()>;
    
    /// Get the current state as key-value pairs
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
    
    /// Convert to Any for dynamic casting
    fn as_any(&self) -> &dyn Any;
}

/// A struct for building Metashrew indexer programs
pub struct MetashrewIndexer<T: Indexer> {
    indexer: T,
}

impl<T: Indexer> MetashrewIndexer<T> {
    /// Create a new MetashrewIndexer
    pub fn new(indexer: T) -> Self {
        Self { indexer }
    }
    
    /// Process a block and update the state
    pub fn process_block(&mut self) -> Result<()> {
        // Load the input data
        let (height, block) = crate::host::load_input()?;
        
        // Process the block
        self.indexer.index_block(height, &block)?;
        
        // Flush the state
        let pairs = self.indexer.flush()?;
        crate::host::flush(&pairs)?;
        
        Ok(())
    }
    
    /// Get a reference to the indexer
    pub fn get_indexer(&self) -> &T {
        &self.indexer
    }
}

/// A simple key-value store for indexer state
pub struct KeyValueStore {
    store: HashMap<Vec<u8>, Vec<u8>>,
}

impl KeyValueStore {
    /// Create a new KeyValueStore
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }
    
    /// Set a value in the store
    pub fn set(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.store.insert(key, value);
    }
    
    /// Get a value from the store
    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.store.get(key)
    }
    
    /// Check if the store contains a key
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.store.contains_key(key)
    }
    
    /// Remove a key from the store
    pub fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.store.remove(key)
    }
    
    /// Get all key-value pairs in the store
    pub fn pairs(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.store
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl Default for KeyValueStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A trait for indexers that can be built as native binaries
pub trait NativeIndexer: Indexer + Default + Clone + Send + Sync + 'static {
    /// Get a map of view function names to their implementations
    fn view_functions(&self) -> HashMap<String, Box<dyn ViewFunctionWrapper>>;
}

/// A trait for wrapping view functions
pub trait ViewFunctionWrapper: Send + Sync {
    /// Execute the view function with the given input
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>>;
}

/// A wrapper for view functions that use serde
pub struct SerdeViewFunctionWrapper<T, I, O>
where
    T: ViewFunction<I, O>,
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize,
{
    view: T,
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<T, I, O> SerdeViewFunctionWrapper<T, I, O>
where
    T: ViewFunction<I, O>,
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize,
{
    /// Create a new SerdeViewFunctionWrapper
    pub fn new(view: T) -> Self {
        Self {
            view,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, I, O> ViewFunctionWrapper for SerdeViewFunctionWrapper<T, I, O>
where
    T: ViewFunction<I, O> + Send + Sync,
    I: for<'de> serde::Deserialize<'de> + Send + Sync,
    O: serde::Serialize + Send + Sync,
{
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>> {
        let input: I = serde_json::from_slice(input)?;
        let output = self.view.execute(input)?;
        let result = serde_json::to_vec(&output)?;
        Ok(result)
    }
}

/// A wrapper for view functions that use Protocol Buffers
pub struct ProtoViewFunctionWrapper<T, I, O>
where
    T: ProtoViewFunction<I, O>,
    I: protobuf::Message,
    O: protobuf::Message,
{
    view: T,
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<T, I, O> ProtoViewFunctionWrapper<T, I, O>
where
    T: ProtoViewFunction<I, O>,
    I: protobuf::Message,
    O: protobuf::Message,
{
    /// Create a new ProtoViewFunctionWrapper
    pub fn new(view: T) -> Self {
        Self {
            view,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, I, O> ViewFunctionWrapper for ProtoViewFunctionWrapper<T, I, O>
where
    T: ProtoViewFunction<I, O> + Send + Sync,
    I: protobuf::Message + Send + Sync,
    O: protobuf::Message + Send + Sync,
{
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>> {
        let input = I::parse_from_bytes(input)?;
        let output = self.view.execute_proto(input)?;
        let result = output.write_to_bytes()?;
        Ok(result)
    }
}
