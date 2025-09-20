//! WASM runtime logic for the alkanes inspector.

use super::types::*;
use anyhow::Result;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(target_arch = "wasm32")]
use alloc::sync::Arc;
#[cfg(target_arch = "wasm32")]
use spin::Mutex;

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
use std::{string::ToString, format, vec, vec::Vec, string::String};

#[cfg(feature = "wasm-inspection")]
use wasmi::{Caller, Config, Engine, Linker, Memory, Store, StoreLimitsBuilder};

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

/// Create a wasmi engine with host functions
#[cfg(feature = "wasm-inspection")]
pub(crate) fn create_engine() -> Engine {
    let mut config = Config::default();
    config.consume_fuel(true);
    Engine::new(&config)
}

/// Create a wasmi store with runtime state
#[cfg(feature = "wasm-inspection")]
pub(crate) fn create_store(engine: &Engine, context: AlkanesRuntimeContext) -> Store<AlkanesState> {
    let state = AlkanesState {
        had_failure: false,
        context: Arc::new(Mutex::new(context)),
        host_calls: Arc::new(Mutex::new(Vec::new())),
        #[cfg(feature = "wasm-inspection")]
        limiter: StoreLimitsBuilder::new().memory_size(16 * 1024 * 1024).build(), // 16MB memory limit
    };
    let mut store = Store::new(engine, state);
    #[cfg(feature = "wasm-inspection")]
    store.limiter(|state| &mut state.limiter);
    store.set_fuel(100_000_000).unwrap(); // Set fuel for execution
    store
}

/// Create host functions for the alkane runtime matching alkanes-rs exactly
#[cfg(feature = "wasm-inspection")]
pub(crate) fn create_host_functions(engine: &Engine) -> Linker<AlkanesState> {
    let mut linker = Linker::new(engine);
    
    // abort - matches alkanes-rs signature
    linker.func_wrap("env", "abort", |mut caller: Caller<'_, AlkanesState>, _: i32, _: i32, _: i32, _: i32| {
        caller.data_mut().had_failure = true;
    }).unwrap();

    // __request_context - matches alkanes-rs signature
    linker.func_wrap("env", "__request_context", |caller: Caller<'_, AlkanesState>| -> i32 {
        let context_guard = lock_mutex!(caller.data().context);
        let serialized = context_guard.serialize();
        serialized.len() as i32
    }).unwrap();

    // __load_context - matches alkanes-rs signature
    linker.func_wrap("env", "__load_context", |mut caller: Caller<'_, AlkanesState>, output: i32| -> i32 {
        let serialized = {
            let context_guard = lock_mutex!(caller.data().context);
            context_guard.serialize()
        };
        
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            
            // Write the serialized context directly (no length prefix)
            if memory.write(&mut caller, output_addr, &serialized).is_ok() {
                return serialized.len() as i32;
            }
        }
        -1
    }).unwrap();

    // __request_storage - matches alkanes-rs signature
    linker.func_wrap("env", "__request_storage", |caller: Caller<'_, AlkanesState>, k: i32| -> i32 {
        let start_time = Instant::now();
        
        // Read the storage key from memory
        let key_str = if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let k_addr = k as usize;
            
            // Read length from ptr - 4 (4 bytes before the pointer)
            if k_addr >= 4 {
                let mut len_bytes = [0u8; 4];
                if memory.read(&caller, k_addr - 4, &mut len_bytes).is_ok() {
                    let len = u32::from_le_bytes(len_bytes) as usize;
                    
                    let mut key_bytes = vec![0u8; len];
                    if memory.read(&caller, k_addr, &mut key_bytes).is_ok() {
                        String::from_utf8_lossy(&key_bytes).to_string()
                    } else {
                        format!("invalid_key_bounds_ptr_{k}_len_{len}")
                    }
                } else {
                    format!("invalid_key_ptr_{k}")
                }
            } else {
                format!("invalid_key_ptr_{k}")
            }
        } else {
            format!("no_memory_export_key_{k}")
        };
        
        // For now, return 0 size but track the call
        let result_size = 0;
        
        // Record the host call
        let host_call = HostCall {
            function_name: "__request_storage".to_string(),
            parameters: vec![format!("key: \"{}\"", key_str)],
            result: format!("size: {result_size}"),
            timestamp_micros: start_time.elapsed().as_micros() as u64,
        };
        
        {
            let mut calls = lock_mutex!(caller.data().host_calls);
            calls.push(host_call);
        }
        
        result_size
    }).unwrap();

    // __load_storage - matches alkanes-rs signature
    linker.func_wrap("env", "__load_storage", |mut caller: Caller<'_, AlkanesState>, k: i32, v: i32| -> i32 {
        let start_time = Instant::now();
        
        // Read the storage key from memory
        let key_str = if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let k_addr = k as usize;
            
            // Read length from ptr - 4 (4 bytes before the pointer)
            if k_addr >= 4 {
                let mut len_bytes = [0u8; 4];
                if memory.read(&caller, k_addr - 4, &mut len_bytes).is_ok() {
                    let len = u32::from_le_bytes(len_bytes) as usize;
                    
                    let mut key_bytes = vec![0u8; len];
                    if memory.read(&caller, k_addr, &mut key_bytes).is_ok() {
                        String::from_utf8_lossy(&key_bytes).to_string()
                    } else {
                        format!("invalid_key_bounds_ptr_{k}_len_{len}")
                    }
                } else {
                    format!("invalid_key_ptr_{k}")
                }
            } else {
                format!("invalid_key_ptr_{k}")
            }
        } else {
            format!("no_memory_export_key_{k}")
        };
        
        // Simulate storage values based on key patterns
        let storage_value = match key_str.as_str() {
            "/position_count" => 42u128.to_le_bytes().to_vec(),
            "/acc_reward_per_share" => 1000000u128.to_le_bytes().to_vec(),
            "/last_reward_block" => 800000u128.to_le_bytes().to_vec(),
            "/last_update_block" => 800001u128.to_le_bytes().to_vec(),
            "/reward_per_block" => 100u128.to_le_bytes().to_vec(),
            "/start_block" => 750000u128.to_le_bytes().to_vec(),
            "/end_reward_block" => 850000u128.to_le_bytes().to_vec(),
            "/total_assets" => 5000000u128.to_le_bytes().to_vec(),
            "/deposit_token_id" => {
                // Return a mock AlkaneId (32 bytes: 16 for block, 16 for tx)
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&1u128.to_le_bytes()); // block
                bytes.extend_from_slice(&100u128.to_le_bytes()); // tx
                bytes
            },
            "/free_mint_contract_id" => {
                // Return a mock AlkaneId (32 bytes: 16 for block, 16 for tx)
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&2u128.to_le_bytes()); // block
                bytes.extend_from_slice(&200u128.to_le_bytes()); // tx
                bytes
            },
             _ if key_str.starts_with("/positions/") => {
                // Simulate a position struct
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&1000u128.to_le_bytes()); // liquidity
                bytes.extend_from_slice(&5000u128.to_le_bytes()); // reward_debt
                bytes
            },
            _ if key_str.starts_with("/registered_children/") => {
                vec![1u8] // Simulate registered child
            },
            _ => vec![], // Empty for unknown keys
        };
        
        // Write the storage value to memory
        let bytes_written = if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let v_addr = v as usize;
            
            // Write length first
            let len_bytes = (storage_value.len() as u32).to_le_bytes();
            if memory.write(&mut caller, v_addr, &len_bytes).is_ok() {
                // Write storage value
                if memory.write(&mut caller, v_addr + 4, &storage_value).is_ok() {
                    storage_value.len() as i32
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };
        
        // Record the host call
        let host_call = HostCall {
            function_name: "__load_storage".to_string(),
            parameters: vec![format!("key: \"{}\"", key_str)],
            result: format!("value: {} bytes ({})", storage_value.len(), hex::encode(&storage_value)),
            timestamp_micros: start_time.elapsed().as_micros() as u64,
        };
        
        {
            let mut calls = lock_mutex!(caller.data().host_calls);
            calls.push(host_call);
        }
        
        bytes_written
    }).unwrap();

    // __height - matches alkanes-rs signature
    linker.func_wrap("env", "__height", |mut caller: Caller<'_, AlkanesState>, output: i32| {
        let height: u64 = 800000; // Placeholder height
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            let height_bytes = height.to_le_bytes();
            
            // Write length first
            let len_bytes = (height_bytes.len() as u32).to_le_bytes();
            if memory.write(&mut caller, output_addr, &len_bytes).is_ok() {
                // Write height data
                let _ = memory.write(&mut caller, output_addr + 4, &height_bytes);
            }
        }
    }).unwrap();

    // __log - matches alkanes-rs signature
    linker.func_wrap("env", "__log", |caller: Caller<'_, AlkanesState>, v: i32| {
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let v_addr = v as usize;
            
            // Read length from ptr - 4 (4 bytes before the pointer)
            if v_addr >= 4 {
                let mut len_bytes = [0u8; 4];
                if memory.read(&caller, v_addr - 4, &mut len_bytes).is_ok() {
                    let len = u32::from_le_bytes(len_bytes) as usize;
                    
                    let mut message_bytes = vec![0u8; len];
                    if memory.read(&caller, v_addr, &mut message_bytes).is_ok() {
                        if let Ok(message) = String::from_utf8(message_bytes) {
                            print!("{message}");
                        }
                    }
                }
            }
        }
    }).unwrap();

    // __balance - matches alkanes-rs signature
    linker.func_wrap("env", "__balance", |mut caller: Caller<'_, AlkanesState>, _who: i32, _what: i32, output: i32| {
        // Return zero balance
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            let zero_balance = 0u128.to_le_bytes();
            
            let len_bytes = (zero_balance.len() as u32).to_le_bytes();
            if memory.write(&mut caller, output_addr, &len_bytes).is_ok() {
                let _ = memory.write(&mut caller, output_addr + 4, &zero_balance);
            }
        }
    }).unwrap();

    // __sequence - matches alkanes-rs signature
    linker.func_wrap("env", "__sequence", |mut caller: Caller<'_, AlkanesState>, output: i32| {
        let sequence: u128 = 0; // Placeholder sequence
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            let seq_bytes = sequence.to_le_bytes();
            
            let len_bytes = (seq_bytes.len() as u32).to_le_bytes();
            if memory.write(&mut caller, output_addr, &len_bytes).is_ok() {
                let _ = memory.write(&mut caller, output_addr + 4, &seq_bytes);
            }
        }
    }).unwrap();

    // __fuel - matches alkanes-rs signature
    linker.func_wrap("env", "__fuel", |mut caller: Caller<'_, AlkanesState>, output: i32| {
        let fuel: u64 = 1000000; // Placeholder fuel
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            let fuel_bytes = fuel.to_le_bytes();
            
            let len_bytes = (fuel_bytes.len() as u32).to_le_bytes();
            if memory.write(&mut caller, output_addr, &len_bytes).is_ok() {
                let _ = memory.write(&mut caller, output_addr + 4, &fuel_bytes);
            }
        }
    }).unwrap();

    // __returndatacopy - matches alkanes-rs signature
    linker.func_wrap("env", "__returndatacopy", |mut caller: Caller<'_, AlkanesState>, output: i32| {
        let returndata = {
            let context_guard = lock_mutex!(caller.data().context);
            context_guard.returndata.clone()
        };
        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
            let output_addr = output as usize;
            
            let len_bytes = (returndata.len() as u32).to_le_bytes();
            if memory.write(&mut caller, output_addr, &len_bytes).is_ok() {
                let _ = memory.write(&mut caller, output_addr + 4, &returndata);
            }
        }
    }).unwrap();

    // __request_transaction - matches alkanes-rs signature
    linker.func_wrap("env", "__request_transaction", |_caller: Caller<'_, AlkanesState>| -> i32 {
        0 // Return 0 size for now
    }).unwrap();

    // __load_transaction - matches alkanes-rs signature
    linker.func_wrap("env", "__load_transaction", |_caller: Caller<'_, AlkanesState>, _output: i32| {
        // Placeholder - do nothing
    }).unwrap();

    // __request_block - matches alkanes-rs signature
    linker.func_wrap("env", "__request_block", |_caller: Caller<'_, AlkanesState>| -> i32 {
        0 // Return 0 size for now
    }).unwrap();

    // __load_block - matches alkanes-rs signature
    linker.func_wrap("env", "__load_block", |_caller: Caller<'_, AlkanesState>, _output: i32| {
        // Placeholder - do nothing
    }).unwrap();

    // __call - matches alkanes-rs signature
    linker.func_wrap("env", "__call", |mut caller: Caller<'_, AlkanesState>, cellpack_ptr: i32, _incoming_alkanes_ptr: i32, _checkpoint_ptr: i32, start_fuel: u64| -> i32 {
        let start_time = Instant::now();
        
        // Try to decode the cellpack to see what alkane is being called
        let call_info = decode_cellpack_info(&mut caller, cellpack_ptr);
        
        // Record the host call
        let host_call = HostCall {
            function_name: "__call".to_string(),
            parameters: vec![
                format!("target: {}", call_info),
                format!("fuel: {}", start_fuel),
            ],
            result: "not_implemented".to_string(),
            timestamp_micros: start_time.elapsed().as_micros() as u64,
        };
        
        {
            let mut calls = lock_mutex!(caller.data().host_calls);
            calls.push(host_call);
        }
        
        -1 // Not implemented
    }).unwrap();

    // __delegatecall - matches alkanes-rs signature
    linker.func_wrap("env", "__delegatecall", |mut caller: Caller<'_, AlkanesState>, cellpack_ptr: i32, _incoming_alkanes_ptr: i32, _checkpoint_ptr: i32, start_fuel: u64| -> i32 {
        let start_time = Instant::now();
        
        let call_info = decode_cellpack_info(&mut caller, cellpack_ptr);
        
        let host_call = HostCall {
            function_name: "__delegatecall".to_string(),
            parameters: vec![
                format!("target: {}", call_info),
                format!("fuel: {}", start_fuel),
            ],
            result: "not_implemented".to_string(),
            timestamp_micros: start_time.elapsed().as_micros() as u64,
        };
        
        {
            let mut calls = lock_mutex!(caller.data().host_calls);
            calls.push(host_call);
        }
        
        -1 // Not implemented
    }).unwrap();

    // __staticcall - matches alkanes-rs signature
    linker.func_wrap("env", "__staticcall", |mut caller: Caller<'_, AlkanesState>, cellpack_ptr: i32, _incoming_alkanes_ptr: i32, _checkpoint_ptr: i32, start_fuel: u64| -> i32 {
        let start_time = Instant::now();
        
        let call_info = decode_cellpack_info(&mut caller, cellpack_ptr);
        
        let host_call = HostCall {
            function_name: "__staticcall".to_string(),
            parameters: vec![
                format!("target: {}", call_info),
                format!("fuel: {}", start_fuel),
            ],
            result: "not_implemented".to_string(),
            timestamp_micros: start_time.elapsed().as_micros() as u64,
        };
        
        {
            let mut calls = lock_mutex!(caller.data().host_calls);
            calls.push(host_call);
        }
        
        -1 // Not implemented
    }).unwrap();
    
    linker
}

/// Helper function to decode cellpack information from memory
#[cfg(feature = "wasm-inspection")]
pub(crate) fn decode_cellpack_info(caller: &mut Caller<'_, AlkanesState>, cellpack_ptr: i32) -> String {
    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
        let ptr_addr = cellpack_ptr as usize;
        
        // Read length from ptr - 4 (4 bytes before the pointer)
        if ptr_addr >= 4 {
            let mut len_bytes = [0u8; 4];
            if memory.read(&mut *caller, ptr_addr - 4, &mut len_bytes).is_ok() {
                let len = u32::from_le_bytes(len_bytes) as usize;
                
                if len >= 32 {
                    // Try to read target AlkaneId (first 32 bytes starting from ptr)
                    let mut target_bytes = [0u8; 32];
                    if memory.read(&mut *caller, ptr_addr, &mut target_bytes).is_ok() {
                        let block = u128::from_le_bytes(target_bytes[0..16].try_into().unwrap_or([0; 16]));
                        let tx = u128::from_le_bytes(target_bytes[16..32].try_into().unwrap_or([0; 16]));
                        
                        // Try to read inputs if available
                        let inputs_info = if len > 32 {
                            let remaining_len = len - 32;
                            let inputs_count = remaining_len / 16; // Each u128 input is 16 bytes
                            format!(" with {inputs_count} inputs")
                        } else {
                            String::new()
                        };
                        
                        return format!("AlkaneId{{block: {block}, tx: {tx}}}{inputs_info}");
                    }
                }
            }
        }
    }
    format!("unknown_cellpack_{cellpack_ptr}")
}

/// Decode ExtendedCallResponse structure from WASM memory
#[cfg(feature = "wasm-inspection")]
pub(crate) fn decode_extended_call_response(store: &Store<AlkanesState>, memory: Memory, ptr: usize) -> Result<(Vec<u8>, Option<String>)> {
    let memory_size = memory.data(store).len();
    
    if ptr < 4 || ptr >= memory_size {
        return Err(anyhow::anyhow!("Response pointer 0x{:x} is invalid (memory size: {})", ptr, memory_size));
    }
    
    // Read length from ptr-4 (4 bytes before the pointer)
    let mut len_bytes = [0u8; 4];
    memory.read(store, ptr - 4, &mut len_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to read response length at 0x{:x}: {:?}", ptr - 4, e))?;
    let response_len = u32::from_le_bytes(len_bytes) as usize;
    
    if response_len == 0 {
        return Ok((vec![], None));
    }
    
    if ptr + response_len > memory_size {
        return Err(anyhow::anyhow!("Response data extends beyond memory bounds: ptr=0x{:x}, len={}, memory_size={}", ptr, response_len, memory_size));
    }
    
    // Read the ExtendedCallResponse structure starting at ptr
    let mut response_bytes = vec![0u8; response_len];
    memory.read(store, ptr, &mut response_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to read ExtendedCallResponse at 0x{:x}: {:?}", ptr, e))?;
    
    // Look for the Solidity error signature pattern
    let mut data_start = 0;
    let mut found_error_sig = false;
    
    for i in 0..response_bytes.len().saturating_sub(4) {
        if response_bytes[i..i+4] == [0x08, 0xc3, 0x79, 0xa0] {
            data_start = i;
            found_error_sig = true;
            break;
        }
    }
    
    if found_error_sig {
        // Extract the error message after the signature
        let message_start = data_start + 4; // Skip the 4-byte signature
        
        if message_start < response_bytes.len() {
            let message_bytes = &response_bytes[message_start..];
            
            // Try to extract readable text
            let mut error_msg = String::new();
            for &byte in message_bytes {
                if (32..=126).contains(&byte) { // Printable ASCII
                    error_msg.push(byte as char);
                } else if byte == 0 {
                    break; // End of string
                }
            }
            
            let clean_msg = error_msg.trim().to_string();
            if !clean_msg.is_empty() {
                return Ok((message_bytes.to_vec(), Some(clean_msg)));
            } else {
                return Ok((message_bytes.to_vec(), Some("Unknown error".to_string())));
            }
        }
    }
    
    // If no error signature found, look for other patterns
    let first_16_zero = response_bytes.len() >= 16 && response_bytes[0..16].iter().all(|&b| b == 0);
    if first_16_zero {
        // Look for data after the header
        if response_bytes.len() > 16 {
            let data_part = &response_bytes[16..];
            
            if data_part.iter().any(|&b| b != 0) {
                // Try to interpret as string
                if let Ok(text) = String::from_utf8(data_part.to_vec()) {
                    let clean_text = text.trim_matches('\0').trim();
                    if !clean_text.is_empty() && clean_text.is_ascii() {
                        return Ok((data_part.to_vec(), None));
                    }
                }
                
                return Ok((data_part.to_vec(), None));
            } else {
                return Ok((vec![], None));
            }
        }
    }
    
    // Fallback: return the raw response data
    Ok((response_bytes, Some("Unknown response format".to_string())))
}

/// Read metadata from WASM memory
#[cfg(feature = "wasm-inspection")]
pub(crate) fn read_metadata_from_memory(store: &Store<AlkanesState>, memory: Memory, ptr: usize) -> Result<AlkaneMetadata> {
    // Get memory size for bounds checking
    let memory_size = memory.data(store).len();
    
    if ptr < 4 || ptr >= memory_size {
        return Err(anyhow::anyhow!("Pointer 0x{:x} is invalid (memory size: {})", ptr, memory_size));
    }
    
    // Read length from ptr-4 (length is stored before the data)
    let mut len_bytes = [0u8; 4];
    memory.read(store, ptr - 4, &mut len_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to read metadata length at 0x{:x}: {:?}", ptr - 4, e))?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    
    if ptr + len > memory_size {
        return Err(anyhow::anyhow!("Metadata extends beyond memory bounds: ptr=0x{:x}, len={}, memory_size={}", ptr, len, memory_size));
    }
    
    // Read metadata bytes starting at ptr
    let mut metadata_bytes = vec![0u8; len];
    memory.read(store, ptr, &mut metadata_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to read metadata bytes at 0x{:x}: {:?}", ptr, e))?;
    
    // Try to parse as JSON first, then fall back to basic parsing
    if let Ok(json_meta) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
        // Extract contract name (could be in "contract" or "name" field)
        let contract_name = json_meta.get("contract")
            .and_then(|v| v.as_str())
            .or_else(|| json_meta.get("name").and_then(|v| v.as_str()))
            .unwrap_or("Unknown")
            .to_string();
        
        // Extract version
        let version = json_meta.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        
        // Extract description
        let description = json_meta.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        // Extract methods with detailed information
        let mut methods = Vec::new();
        
        if let Some(methods_array) = json_meta.get("methods").and_then(|v| v.as_array()) {
            for method in methods_array {
                let name = method.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let opcode = method.get("opcode")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u128;
                
                let params = method.get("params")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|p| p.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_else(Vec::new);
                
                let returns = method.get("returns")
                    .and_then(|v| v.as_str())
                    .unwrap_or("void")
                    .to_string();
                
                methods.push(AlkaneMethod {
                    name,
                    opcode,
                    params,
                    returns,
                });
            }
        }
        
        Ok(AlkaneMetadata {
            name: contract_name,
            version,
            description,
            methods,
        })
    } else {
        // Fallback to basic metadata
        Ok(AlkaneMetadata {
            name: "Unknown".to_string(),
            version: "0.0.0".to_string(),
            description: None,
            methods: vec![],
        })
    }
}