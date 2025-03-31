//! View functionality for Metashrew.
//!
//! This module provides functionality for creating view functions in Metashrew indexer programs.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// A trait for types that can be used as view functions in Metashrew
pub trait ViewFunction<Input, Output> {
    /// Execute the view function with the given input
    fn execute(&self, input: Input) -> Result<Output>;
}

/// A trait for types that can be used as view functions with Protocol Buffer messages
pub trait ProtoViewFunction<Input, Output>: Any {
    /// Execute the view function with the given input
    fn execute_proto(&self, input: Input) -> Result<Output>;
}

/// A struct for building Metashrew view functions
pub struct MetashrewView<T> {
    view: T,
}

impl<T> MetashrewView<T> {
    /// Create a new MetashrewView
    pub fn new(view: T) -> Self {
        Self { view }
    }
}

impl<T> MetashrewView<T>
where
    T: ViewFunction<Vec<u8>, Vec<u8>>,
{
    /// Process a view request
    pub fn process_view(&self) -> Result<Vec<u8>> {
        // Load the input data
        let (_height, input_bytes) = crate::host::load_input()?;
        
        // Execute the view function
        let output = self.view.execute(input_bytes)?;
        
        Ok(output)
    }
}

/// A struct for building Metashrew view functions with Protocol Buffer messages
pub struct MetashrewProtoView<T> {
    view: T,
}

impl<T> MetashrewProtoView<T> {
    /// Create a new MetashrewProtoView
    pub fn new(view: T) -> Self {
        Self { view }
    }
}

impl<T> MetashrewProtoView<T>
where
    T: ProtoViewFunction<Vec<u8>, Vec<u8>>,
{
    /// Process a view request
    pub fn process_view(&self) -> Result<Vec<u8>> {
        // Load the input data
        let (_height, input_bytes) = crate::host::load_input()?;
        
        // Execute the view function
        let output = self.view.execute_proto(input_bytes)?;
        
        Ok(output)
    }
}

/// A helper function to return a view result
pub fn return_view_result(result: &[u8]) -> i32 {
    let len = result.len() as u32;
    let mut buffer = Vec::with_capacity(4 + result.len());
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(result);
    
    buffer.as_ptr() as i32
}

/// A helper function to parse view input
pub fn parse_view_input<T: for<'de> Deserialize<'de>>() -> Result<T> {
    let (_height, input_bytes) = crate::host::load_input()?;
    
    serde_json::from_slice(&input_bytes)
        .map_err(|e| anyhow!("Failed to deserialize input: {}", e))
}

/// A helper function to parse Protocol Buffer view input
pub fn parse_proto_view_input() -> Result<Vec<u8>> {
    let (_height, input_bytes) = crate::host::load_input()?;
    
    Ok(input_bytes)
}

/// Execute a view function by name
pub fn execute_view_function<T>(
    indexer: &T,
    function_name: &str,
    input_bytes: &[u8],
) -> Result<Vec<u8>> {
    // This is a simplified implementation that delegates to the execute_proto method
    // In a real implementation, this would need to handle different types of view functions
    match function_name {
        _ => {
            // For now, just return an empty vector
            // In a real implementation, this would call the appropriate method on the indexer
            Ok(Vec::new())
        }
    }
}
