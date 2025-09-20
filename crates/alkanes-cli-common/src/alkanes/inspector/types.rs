//! Data structures for the alkanes inspector.

use serde::{Deserialize, Serialize};
use crate::alkanes::types::AlkaneId;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(target_arch = "wasm32")]
use alloc::sync::Arc;
#[cfg(target_arch = "wasm32")]
use spin::Mutex;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;


#[cfg(not(feature = "std"))]
use alloc::{string::ToString, format, vec, vec::Vec, boxed::Box, string::String};
#[cfg(feature = "std")]
use std::{vec, vec::Vec, boxed::Box, string::String};

#[cfg(feature = "wasm-inspection")]
use wasmi::StoreLimits;

// Re-export alkanes support types for cross-platform compatibility
pub use alkanes_support::{
    id::AlkaneId as AlkanesAlkaneId,
    parcel::AlkaneTransferParcel,
    trace::Trace,
};

/// Simple message context parcel for alkane execution
#[derive(Default, Clone, Debug)]
pub struct MessageContextParcel {
    pub vout: u32,
    pub height: u64,
    pub calldata: Vec<u8>,
}

/// Alkanes runtime context for VM execution - matches alkanes-rs exactly
#[derive(Default, Clone)]
pub struct AlkanesRuntimeContext {
    pub myself: AlkanesAlkaneId,
    pub caller: AlkanesAlkaneId,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub returndata: Vec<u8>,
    pub inputs: Vec<u128>,
    pub message: Box<MessageContextParcel>,
    pub trace: Trace,
}

impl AlkanesRuntimeContext {
    pub fn from_cellpack_inputs(inputs: Vec<u128>) -> Self {
        let message = MessageContextParcel::default();
        Self {
            message: Box::new(message),
            returndata: vec![],
            incoming_alkanes: AlkaneTransferParcel::default(),
            myself: AlkanesAlkaneId::default(),
            caller: AlkanesAlkaneId::default(),
            trace: Trace::default(),
            inputs,
        }
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let flattened = self.flatten();
        let mut result = Vec::new();
        for value in flattened {
            result.extend_from_slice(&value.to_le_bytes());
        }
        result
    }
    
    pub fn flatten(&self) -> Vec<u128> {
        let mut result = vec![
            self.myself.block,
            self.myself.tx,
            self.caller.block,
            self.caller.tx,
            self.message.vout as u128,
            self.incoming_alkanes.0.len() as u128,
        ];
        for incoming in &self.incoming_alkanes.0 {
            result.push(incoming.id.block);
            result.push(incoming.id.tx);
            result.push(incoming.value);
        }
        for input in self.inputs.clone() {
            result.push(input);
        }
        result
    }
}

/// VM state for alkanes execution
pub struct AlkanesState {
    pub had_failure: bool,
    pub context: Arc<Mutex<AlkanesRuntimeContext>>,
    pub host_calls: Arc<Mutex<Vec<HostCall>>>,
    #[cfg(feature = "wasm-inspection")]
    pub limiter: StoreLimits,
}

/// Configuration for alkanes inspection
#[derive(Debug, Clone)]
pub struct InspectionConfig {
    pub disasm: bool,
    pub fuzz: bool,
    pub fuzz_ranges: Option<String>,
    pub meta: bool,
    pub codehash: bool,
    pub raw: bool,
}

/// Result of alkanes inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    pub alkane_id: AlkaneId,
    pub bytecode_length: usize,
    pub codehash: Option<String>,
    pub disassembly: Option<String>,
    pub metadata: Option<AlkaneMetadata>,
    pub metadata_error: Option<String>,
    pub fuzzing_results: Option<FuzzingResults>,
}

/// Alkane metadata extracted from __meta export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub methods: Vec<AlkaneMethod>,
}

/// Method information from alkane metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneMethod {
    pub name: String,
    pub opcode: u128,
    pub params: Vec<String>,
    pub returns: String,
}

/// Results of fuzzing analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzingResults {
    pub total_opcodes_tested: usize,
    pub opcodes_filtered_out: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub implemented_opcodes: Vec<u128>,
    pub opcode_results: Vec<ExecutionResult>,
}

/// Result of opcode execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub return_value: Option<i32>,
    pub return_data: Vec<u8>,
    pub error: Option<String>,
    pub execution_time_micros: u64,
    pub opcode: u128,
    pub host_calls: Vec<HostCall>,
}

/// Record of a host function call made during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCall {
    pub function_name: String,
    pub parameters: Vec<String>,
    pub result: String,
    pub timestamp_micros: u64,
}

impl HostCall {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(function_name: String, parameters: Vec<String>, result: String, timestamp: Instant) -> Self {
        Self {
            function_name,
            parameters,
            result,
            timestamp_micros: timestamp.elapsed().as_micros() as u64,
        }
    }
    
    #[cfg(target_arch = "wasm32")]
    pub fn new(function_name: String, parameters: Vec<String>, result: String, _timestamp: u64) -> Self {
        Self {
            function_name,
            parameters,
            result,
            timestamp_micros: 0, // WASM doesn't have precise timing
        }
    }
}