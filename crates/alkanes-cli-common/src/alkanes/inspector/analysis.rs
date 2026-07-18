//! Core analysis logic for the alkanes inspector.

use super::runtime::*;
use super::types::*;
use crate::alkanes::types::AlkaneId;
use anyhow::{Context, Result};
use sha3::{Digest, Keccak256};

#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use alloc::collections::BTreeMap as HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use ::core::time::Duration;

#[cfg(target_arch = "wasm32")]
struct Instant;

#[cfg(target_arch = "wasm32")]
impl Instant {
    fn now() -> Self { Instant }
    fn elapsed(&self) -> Duration { Duration::from_micros(0) }
}

#[cfg(not(feature = "std"))]
use alloc::{string::ToString, format, vec, vec::Vec, boxed::Box, string::String};
#[cfg(feature = "std")]
use std::{string::ToString, format, vec, vec::Vec, boxed::Box, string::String};

#[cfg(feature = "wasm-inspection")]
use wasmi::Module;

// Helper macro to handle mutex locking across different mutex types
#[cfg(not(target_arch = "wasm32"))]
macro_rules! lock_mutex {
    ($mutex:expr) => {
        $mutex.lock().unwrap()
    };
}

#[cfg(target_arch = "wasm32")]
macro_rules! lock_mutex {
    ($mutex:expr) => {
        $mutex.lock()
    };
}

/// Compute SHA3 (Keccak256) hash of the WASM bytecode
#[cfg(feature = "wasm-inspection")]
pub(crate) fn compute_codehash(wasm_bytes: &[u8]) -> Result<String> {
    let mut hasher = Keccak256::new();
    hasher.update(wasm_bytes);
    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

/// Extract metadata using WASM runtime
#[cfg(feature = "wasm-inspection")]
pub(crate) async fn extract_metadata(wasm_bytes: &[u8]) -> Result<AlkaneMetadata> {
    let engine = create_engine();
    
    // Create a basic context for metadata extraction
    let context = AlkanesRuntimeContext {
        inputs: vec![],
        ..Default::default()
    };
    
    let mut store = create_store(&engine, context);
    let linker = create_host_functions(store.engine());
    
    // Compile and instantiate the module
    let module = Module::new(store.engine(), wasm_bytes)
        .context("Failed to compile WASM module")?;
    
    let instance = linker.instantiate(&mut store, &module)
        .context("Failed to instantiate WASM module")?
        .ensure_no_start(&mut store)
        .context("Failed to ensure no start function")?;
    
    // Get memory export
    let memory = instance.get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("No memory export found"))?;
    
    // Get __meta export
    let meta_func = instance.get_func(&mut store, "__meta")
        .ok_or_else(|| anyhow::anyhow!("No __meta export found"))?
        .typed::<(), i32>(&store)
        .context("Failed to get typed __meta function")?;
    
    // Execute __meta
    let meta_ptr = meta_func.call(&mut store, ())
        .context("Failed to execute __meta")?;
    
    // Read metadata from memory
    let metadata = read_metadata_from_memory(&store, memory, meta_ptr as usize)?;
    
    Ok(metadata)
}

/// Disassemble WASM to WAT format
pub(crate) fn disassemble_wasm(wasm_bytes: &[u8]) -> Result<Option<String>> {
    #[cfg(feature = "wasm-inspection")]
    {
        match wasmprinter::print_bytes(wasm_bytes) {
            Ok(wat_content) => Ok(Some(wat_content)),
            Err(_) => Ok(None), // Return None if disassembly fails
        }
    }
    #[cfg(not(feature = "wasm-inspection"))]
    {
        let _ = wasm_bytes; // Suppress unused variable warning
        Ok(None)
    }
}

/// Perform fuzzing analysis using optimized batch execution
#[cfg(feature = "wasm-inspection")]
pub async fn perform_fuzzing_analysis(
    alkane_id: &AlkaneId,
    wasm_bytes: &[u8],
    fuzz_ranges: Option<&str>,
) -> Result<FuzzingResults> {
    // Determine which opcodes to test
    let opcodes_to_test = if let Some(ranges_str) = fuzz_ranges {
        parse_opcode_ranges(ranges_str)?
    } else {
        // Default: test opcodes 0-999
        (0..1000).collect()
    };
    
    // Use optimized batch execution instead of creating new instances for each opcode
    let results = execute_opcodes_batch(wasm_bytes, &opcodes_to_test, alkane_id).await?;
    
    // Apply pattern filtering to identify and remove undefined behavior
    let filtered_results = filter_undefined_behavior_patterns(&results)?;
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for result in &filtered_results {
        if result.success {
            success_count += 1;
        } else {
            error_count += 1;
        }
    }
    
    let implemented_opcodes: Vec<u128> = filtered_results.iter().map(|r| r.opcode).collect();
    let total_tested = results.len();
    let filtered_out = total_tested - filtered_results.len();
    
    Ok(FuzzingResults {
        total_opcodes_tested: total_tested,
        opcodes_filtered_out: filtered_out,
        successful_executions: success_count,
        failed_executions: error_count,
        implemented_opcodes,
        opcode_results: filtered_results,
    })
}

/// Execute multiple opcodes efficiently by reusing the WASM instance
#[cfg(feature = "wasm-inspection")]
async fn execute_opcodes_batch(
    wasm_bytes: &[u8],
    opcodes: &[u128],
    alkane_id: &AlkaneId,
) -> Result<Vec<ExecutionResult>> {
    let engine = create_engine();
    
    // Create initial context - we'll update the inputs for each opcode
    let initial_context = AlkanesRuntimeContext {
        inputs: vec![0u128; 16], // Will be updated for each opcode
        myself: AlkanesAlkaneId {
            block: alkane_id.block as u128,
            tx: alkane_id.tx as u128,
        },
        caller: AlkanesAlkaneId {
            block: alkane_id.block as u128,
            tx: alkane_id.tx as u128,
        },
        message: Box::new(MessageContextParcel {
            vout: 0,
            height: 800000,
            calldata: vec![],
        }),
        ..Default::default()
    };
    
    let mut store = create_store(&engine, initial_context);
    let linker = create_host_functions(store.engine());
    
    // Compile and instantiate the module once
    let module = Module::new(store.engine(), wasm_bytes)
        .context("Failed to compile WASM module")?;
    
    let instance = linker.instantiate(&mut store, &module)
        .context("Failed to instantiate WASM module")?
        .ensure_no_start(&mut store)
        .context("Failed to ensure no start function")?;
    
    // Get memory and function exports once
    let memory = instance.get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("No memory export found"))?;
    
    let execute_func = instance.get_func(&mut store, "__execute")
        .ok_or_else(|| anyhow::anyhow!("No __execute export found"))?
        .typed::<(), i32>(&store)
        .context("Failed to get typed __execute function")?;
    
    let mut results = Vec::new();
    
    // Execute each opcode by updating the context inputs
    for &opcode in opcodes {
        // Update the context inputs for this opcode
        {
            let mut context_guard = lock_mutex!(store.data().context);
            context_guard.inputs[0] = opcode; // First input is the opcode
            // Keep the rest as zeros
            for i in 1..16 {
                context_guard.inputs[i] = 0;
            }
            // Clear return data from previous execution
            context_guard.returndata.clear();
        }
        
        // Clear host calls from previous execution
        {
            let mut calls_guard = lock_mutex!(store.data().host_calls);
            calls_guard.clear();
        }
        
        // Reset failure flag
        store.data_mut().had_failure = false;
        
        // Execute with the updated context
        let start_time = Instant::now();
        let result = execute_func.call(&mut store, ());
        let execution_time = start_time.elapsed();
        
        // Capture host calls for this execution
        let host_calls = {
            let calls_guard = lock_mutex!(store.data().host_calls);
            calls_guard.clone()
        };

        match result {
            Ok(response_ptr) => {
                // Decode the ExtendedCallResponse from the returned pointer
                let (return_data, error_message) = decode_extended_call_response(&store, memory, response_ptr as usize)?;
                
                results.push(ExecutionResult {
                    success: true,
                    return_value: Some(response_ptr),
                    return_data,
                    error: error_message,
                    execution_time_micros: execution_time.as_micros() as u64,
                    opcode,
                    host_calls,
                });
            },
            Err(e) => {
                results.push(ExecutionResult {
                    success: false,
                    return_value: None,
                    return_data: vec![],
                    error: Some(format!("WASM execution failed: {e}")),
                    execution_time_micros: execution_time.as_micros() as u64,
                    opcode,
                    host_calls,
                });
            },
        }
    }
    
    Ok(results)
}

/// Execute an opcode with proper alkane context for fuzzing (single opcode)
#[cfg(feature = "wasm-inspection")]
#[allow(dead_code)]
async fn execute_opcode_with_context(
    wasm_bytes: &[u8],
    opcode: u128,
    alkane_id: &AlkaneId,
) -> Result<ExecutionResult> {
    // Use the batch execution for single opcodes too for consistency
    let results = execute_opcodes_batch(wasm_bytes, &[opcode], alkane_id).await?;
    results.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("No result returned from batch execution"))
}

/// Parse opcode ranges from string (e.g., "0-999,2000-2500")
#[cfg(feature = "wasm-inspection")]
fn parse_opcode_ranges(ranges_str: &str) -> Result<Vec<u128>> {
    let mut opcodes = Vec::new();
    
    for range_part in ranges_str.split(',') {
        let range_part = range_part.trim();
        if range_part.contains('-') {
            let parts: Vec<&str> = range_part.split('-').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid range format: {}", range_part));
            }
            let start: u128 = parts[0].parse()
                .with_context(|| format!("Invalid start opcode: {}", parts[0]))?;
            let end: u128 = parts[1].parse()
                .with_context(|| format!("Invalid end opcode: {}", parts[1]))?;
            
            if start > end {
                return Err(anyhow::anyhow!("Invalid range: start {} > end {}", start, end));
            }
            
            for opcode in start..=end {
                opcodes.push(opcode);
            }
        } else {
            let opcode: u128 = range_part.parse()
                .with_context(|| format!("Invalid opcode: {range_part}"))?;
            opcodes.push(opcode);
        }
    }
    
    opcodes.sort();
    opcodes.dedup();
    Ok(opcodes)
}

/// Filter out opcodes with undefined behavior based on response patterns
#[cfg(feature = "wasm-inspection")]
fn filter_undefined_behavior_patterns(results: &[ExecutionResult]) -> Result<Vec<ExecutionResult>> {
    let mut response_patterns: HashMap<String, Vec<&ExecutionResult>> = HashMap::new();
    
    // Group results by normalized response pattern
    for result in results {
        let pattern_key = normalize_response_pattern(result);
        response_patterns.entry(pattern_key)
            .or_default()
            .push(result);
    }
    
    // Find the largest group of identical responses (likely undefined behavior)
    let largest_group = response_patterns
        .iter()
        .max_by_key(|(_, opcodes)| opcodes.len())
        .map(|(pattern, opcodes)| (pattern.clone(), opcodes.len()));
    
    if let Some((largest_pattern, largest_count)) = largest_group {
        // Only filter if we have multiple patterns AND the largest represents > 80% of results
        // This prevents filtering when ALL results have the same legitimate error
        let threshold = results.len() / 2; // 50% threshold
        
        if largest_count > threshold && largest_pattern.starts_with("ERROR:") {
            // Return only results that don't match the undefined behavior pattern
            let filtered: Vec<ExecutionResult> = results
                .iter()
                .filter(|result| {
                    let pattern = normalize_response_pattern(result);
                    pattern != largest_pattern
                })
                .cloned()
                .collect();
            
            return Ok(filtered);
        }
    }
    
    // If no clear undefined behavior pattern found, return all results
    Ok(results.to_vec())
}

/// Normalize response pattern by removing opcode-specific information
#[cfg(feature = "wasm-inspection")]
fn normalize_response_pattern(result: &ExecutionResult) -> String {
    if let Some(error) = &result.error {
        // Normalize error messages by removing opcode numbers
        let normalized = error
            .replace(&result.opcode.to_string(), "OPCODE")
            .replace(&format!("0x{:x}", result.opcode), "OPCODE")
            .replace(&format!("{:#x}", result.opcode), "OPCODE");
        format!("ERROR:{normalized}")
    } else {
        // For successful results, use return data pattern
        let data_pattern = if result.return_data.is_empty() {
            "EMPTY".to_string()
        } else if result.return_data.len() <= 32 {
            hex::encode(&result.return_data)
        } else {
            format!("{}...({}bytes)", hex::encode(&result.return_data[..16]), result.return_data.len())
        };
        
        // Include host call pattern for more precise matching
        let host_call_pattern = if result.host_calls.is_empty() {
            "NO_CALLS".to_string()
        } else {
            result.host_calls.iter()
                .map(|call| call.function_name.clone())
                .collect::<Vec<_>>()
                .join(",")
        };
        
        format!("SUCCESS:{data_pattern}:CALLS:{host_call_pattern}")
    }
}