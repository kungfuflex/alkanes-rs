//! alkanes-gpu: GPU-accelerated WASM execution for the alkanes indexer.
//!
//! Uses wgpu + WGSL compute shaders to execute independent contract
//! messages in parallel on the GPU, with tiered fallback to CPU threads
//! and sequential wasmi execution.

pub mod device;
pub mod host;
pub mod pipeline;
pub mod tracking;
pub mod types;
pub mod wasm_parser;

use anyhow::Result;
use bytemuck::Zeroable;
use device::{ComputePipeline, GpuDevice};
use types::*;

/// The embedded WGSL shader source.
pub const SHADER_SOURCE: &str = include_str!("shader.wgsl");

/// Per-thread WASM memory size in u32 words (2 MB = 524288 words).
const WASM_MEMORY_WORDS_PER_THREAD: usize = 4194304; // 32 pages = 2MB
/// Per-thread execution state size in u32 words.
/// Must match THREAD_STATE_SIZE in shader.wgsl:
///   STACK_SIZE(512) + LOCALS_SIZE(256) + MAX_CALL_FRAMES(64)*4 + MAX_LABELS(128)*3 + SCALARS(12) + CONTRACT_STATE(22)
const THREAD_STATE_WORDS: usize = 512 + 256 + 64 * 4 + 128 * 3 + 12 + 22;

/// Top-level GPU pipeline for alkanes message execution.
pub struct AlkanesGpu {
    device: GpuDevice,
    pipeline: ComputePipeline,
}

impl AlkanesGpu {
    /// Initialize the GPU pipeline.
    pub fn new() -> Result<Self> {
        let device = GpuDevice::new()?;
        let pipeline = device.create_pipeline(SHADER_SOURCE, "main")?;
        log::info!(
            "AlkanesGpu initialized on {}",
            device.adapter_info.name
        );
        Ok(Self { device, pipeline })
    }

    /// Execute a shard of WASM contract messages on the GPU.
    ///
    /// `input_buffer` should contain:
    ///   - ShardHeader (8 u32s): message_count, kv_count, block_height,
    ///     base_fuel_lo, base_fuel_hi, bytecode_len, import_count, entry_pc
    ///   - Bytecode (packed u32 words)
    ///   - Message data
    ///   - K/V pairs
    ///
    /// Returns per-message results. Messages with `ejected != 0` should
    /// be re-executed on CPU.
    pub fn execute_shard_raw(
        &self,
        input_buffer: &[u32],
        message_count: usize,
    ) -> Result<Vec<GpuMessageResult>> {
        let input_buffer: &[u8] = bytemuck::cast_slice(input_buffer);
        // Output: one GpuMessageResult per message
        let output_size = message_count * std::mem::size_of::<GpuMessageResult>();

        // WASM memory: 1MB per thread, 64 threads max per workgroup
        let thread_count = std::cmp::min(message_count, 64);
        let wasm_mem_size = thread_count * WASM_MEMORY_WORDS_PER_THREAD * 4;
        let thread_state_size = thread_count * THREAD_STATE_WORDS * 4;

        let workgroups = ((message_count + 63) / 64) as u32;

        let output_bytes = self.device.dispatch(
            &self.pipeline,
            input_buffer,
            output_size,
            wasm_mem_size,
            thread_state_size,
            workgroups,
        )?;

        // Copy into aligned buffer to satisfy GpuMessageResult alignment
        let result_size = std::mem::size_of::<GpuMessageResult>();
        let mut results = Vec::with_capacity(message_count);
        for i in 0..message_count {
            let offset = i * result_size;
            let mut result = GpuMessageResult::zeroed();
            let result_bytes = bytemuck::bytes_of_mut(&mut result);
            result_bytes.copy_from_slice(&output_bytes[offset..offset + result_size]);
            results.push(result);
        }

        Ok(results)
    }

    /// Simple shard execution with a header only (for testing).
    pub fn execute_shard(
        &self,
        header: &ShardHeader,
        _messages: &[GpuMessageInput],
        _kv_pairs: &[GpuKvPair],
    ) -> Result<Vec<GpuMessageResult>> {
        // Convert header fields to u32 vec directly to guarantee alignment
        let input: Vec<u32> = vec![
            header.message_count,
            header.kv_count,
            header.block_height,
            header.base_fuel,
            header.base_fuel_hi,
            0, // bytecode_len
            0, // import_count
            0, // entry_pc
            0, // func_count
            13, // func_table_offset (right after header)
            1, // contract_count
            0, // contract_table_offset
            13, // import_map_offset (right after header, no bytecode)
        ];
        self.execute_shard_raw(&input, header.message_count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal input buffer with the shader's header layout.
    fn build_input(
        message_count: u32,
        kv_count: u32,
        block_height: u32,
        fuel: u32,
        bytecode: &[u8],
        import_count: u32,
        entry_pc: u32,
    ) -> Vec<u32> {
        let bytecode_len = bytecode.len() as u32;
        // Pack bytecode bytes into u32 words (little-endian)
        let word_count = ((bytecode.len() + 3) / 4) as usize;
        let mut bytecode_words: Vec<u32> = vec![0u32; word_count];
        for (i, &b) in bytecode.iter().enumerate() {
            bytecode_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }

        let func_table_offset = 17 + bytecode_words.len() as u32;

        let mut input: Vec<u32> = vec![
            message_count,
            kv_count,
            block_height,
            fuel,
            0,
            bytecode_len,
            import_count,
            entry_pc,
            0,
            func_table_offset,
            1, // contract_count (1 = primary only)
            0, // contract_table_offset (unused when count=1)
            0, // import_map_offset — placeholder, filled below
            0, // globals_count
            0, // globals_offset (none)
            0, // data_segments_count
            0, // data_segments_offset (none)
        ];
        input.extend_from_slice(&bytecode_words);
        // Identity import map (import index i -> host function i)
        let import_map_offset = input.len() as u32;
        input[12] = import_map_offset;
        for i in 0..import_count {
            input.push(i);
        }
        // Set globals_offset and data_segments_offset to end of import map
        input[14] = input.len() as u32;
        input[16] = input.len() as u32;
        input
    }

    #[test]
    fn test_gpu_device_init_and_dispatch() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        let input_bytes = build_input(4, 0, 100, 1_000_000, &[], 0, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 4).unwrap();
        assert_eq!(results.len(), 4);

        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.success, 1, "message {} should succeed", i);
            assert_eq!(r.ejected, 0, "message {} should not be ejected", i);
        }
    }

    #[test]
    fn test_simple_wasm_i32_const() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Bytecode: i32.const 42, end
        let bytecode: Vec<u8> = vec![0x41, 0x2A, 0x0B];
        let input_bytes = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].success, 1, "should succeed");
        assert_eq!(results[0].ejected, 0, "should not eject");
        assert!(
            results[0].gas_used_lo < 1_000_000,
            "fuel should be consumed: remaining={}",
            results[0].gas_used_lo
        );
    }

    #[test]
    fn test_i32_arithmetic() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Bytecode: i32.const 10, i32.const 32, i32.add, end
        // Result should be 42 on stack
        let bytecode: Vec<u8> = vec![
            0x41, 0x0A, // i32.const 10
            0x41, 0x20, // i32.const 32
            0x6A,       // i32.add
            0x0B,       // end
        ];
        let input_bytes = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results[0].success, 1, "should succeed");
        assert_eq!(results[0].ejected, 0, "should not eject");
    }

    #[test]
    fn test_control_flow_if_else() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Bytecode: i32.const 1, if [void], i32.const 42, else, i32.const 99, end, end
        let bytecode: Vec<u8> = vec![
            0x41, 0x01, // i32.const 1 (true)
            0x04, 0x40, // if [void]
            0x41, 0x2A, // i32.const 42
            0x05,       // else
            0x41, 0x63, // i32.const 99
            0x0B,       // end (if)
            0x0B,       // end (func)
        ];
        let input_bytes = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results[0].success, 1, "should succeed");
        assert_eq!(results[0].ejected, 0, "should not eject");
    }

    #[test]
    fn test_host_function_ejects_on_extcall() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Bytecode: call 15 (HOST_CALL = extcall), end
        // import_count = 18 (all host functions), func 15 = HOST_CALL
        let bytecode: Vec<u8> = vec![
            0x10, 0x0F, // call 15 (HOST_CALL)
            0x0B,       // end
        ];
        let input_bytes = build_input(1, 0, 100, 1_000_000, &bytecode, 18, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results[0].success, 0, "should fail (ejected)");
        assert_eq!(results[0].ejected, 1, "should be ejected");
        assert_eq!(results[0].ejection_reason, 5, "reason should be EXTCALL (5)");
    }


    #[test]
    fn test_extcall_extracts_target_alkane_id() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Build bytecode that:
        // 1. Writes a mock cellpack at WASM memory address 100
        //    (AlkaneId: block=42, tx=7 — two u128 LE values)
        // 2. Pushes HOST_CALL args onto stack
        // 3. Calls HOST_CALL (func 15)
        //
        // We need to write the cellpack data into WASM memory first,
        // then push the args and call.
        //
        // Memory layout at addr 100: [42 as u128 LE][7 as u128 LE]
        // Bytecode:
        //   i32.const 100   (dest addr)
        //   i32.const 42    (block value to store)
        //   i32.store 0 0   (store block low word at addr 100)
        //   -- push args for HOST_CALL --
        //   i32.const 100   (cellpack_ptr)
        //   i32.const 0     (incoming_alkanes_ptr)
        //   i32.const 0     (checkpoint_ptr)
        //   i64.const 0     (start_fuel)
        //   call 15          (HOST_CALL)
        //   end

        let bytecode: Vec<u8> = vec![
            // Store block=42 at memory offset 100 (little-endian u32)
            0x41, 0xe4, 0x00,       // i32.const 100
            0x41, 0x2A,             // i32.const 42
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Store tx=7 at memory offset 116 (100 + 16)
            0x41, 0xf4, 0x00,       // i32.const 116
            0x41, 0x07,             // i32.const 7
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Push HOST_CALL args (i64 as two i32 words on the stack)
            0x41, 0xe4, 0x00,       // i32.const 100  (cellpack_ptr)
            0x41, 0x00,             // i32.const 0    (incoming_alkanes_ptr)
            0x41, 0x00,             // i32.const 0    (checkpoint_ptr)
            0x41, 0x00,             // i32.const 0    (start_fuel lo)
            0x41, 0x00,             // i32.const 0    (start_fuel hi)
            0x10, 0x0F,             // call 15        (HOST_CALL)
            0x0B,                   // end
        ];

        let input_bytes = build_input(1, 0, 100, 1_000_000, &bytecode, 18, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results[0].ejected, 1, "should be ejected");
        assert_eq!(results[0].ejection_reason, 5, "reason should be EXTCALL");
        assert_eq!(results[0].return_data_len, 32, "should have 32-byte return data");

        // Extract AlkaneId from return_data
        let block = u128::from_le_bytes(results[0].return_data[0..16].try_into().unwrap());
        let tx = u128::from_le_bytes(results[0].return_data[16..32].try_into().unwrap());
        assert_eq!(block, 42, "target block should be 42");
        assert_eq!(tx, 7, "target tx should be 7");
    }

    #[test]
    fn test_fuel_exhaustion() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Bytecode: loop, br 0 (infinite loop), end — should exhaust fuel
        let bytecode: Vec<u8> = vec![
            0x03, 0x40, // loop [void]
            0x0C, 0x00, // br 0 (back to loop)
            0x0B,       // end
            0x0B,       // end (func)
        ];
        // Very low fuel so it exhausts quickly
        let input_bytes = build_input(1, 0, 100, 10, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input_bytes, 1).unwrap();

        assert_eq!(results[0].success, 0, "should fail");
        assert_eq!(results[0].ejected, 1, "should be ejected");
        assert_eq!(results[0].ejection_reason, 6, "reason should be FUEL_EXHAUSTED (6)");
    }
}

#[cfg(test)]
mod tests_i64 {
    use super::*;

    /// Build a minimal input buffer with the shader header layout.
    fn build_input(
        message_count: u32,
        kv_count: u32,
        block_height: u32,
        fuel: u32,
        bytecode: &[u8],
        import_count: u32,
        entry_pc: u32,
    ) -> Vec<u32> {
        let bytecode_len = bytecode.len() as u32;
        let word_count = ((bytecode.len() + 3) / 4) as usize;
        let mut bytecode_words: Vec<u32> = vec![0u32; word_count];
        for (i, &b) in bytecode.iter().enumerate() {
            bytecode_words[i / 4] |= (b as u32) << ((i % 4) * 8);
        }
        let func_table_offset = 17 + bytecode_words.len() as u32;
        let mut input: Vec<u32> = vec![
            message_count, kv_count, block_height, fuel, 0,
            bytecode_len, import_count, entry_pc, 0, func_table_offset,
            1, // contract_count (1 = primary only)
            0, // contract_table_offset (unused when count=1)
            0, // import_map_offset — placeholder
            0, // globals_count
            0, // globals_offset
            0, // data_segments_count
            0, // data_segments_offset
        ];
        input.extend_from_slice(&bytecode_words);
        // Identity import map
        let import_map_offset = input.len() as u32;
        input[12] = import_map_offset;
        for i in 0..import_count {
            input.push(i);
        }
        // Set globals_offset and data_segments_offset to end of import map
        input[14] = input.len() as u32;
        input[16] = input.len() as u32;
        input
    }

    /// Encode an i64 value as signed LEB128.
    fn leb128_i64(mut val: i64) -> Vec<u8> {
        let mut result = Vec::new();
        loop {
            let mut byte = (val & 0x7f) as u8;
            val >>= 7;
            let more = !(((val == 0) && (byte & 0x40 == 0)) ||
                         ((val == -1) && (byte & 0x40 != 0)));
            if more {
                byte |= 0x80;
            }
            result.push(byte);
            if !more { break; }
        }
        result
    }

    #[test]
    fn test_i64_const_small() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = vec![0x42];
        bytecode.extend_from_slice(&leb128_i64(42));
        bytecode.push(0xA7); // i32.wrap_i64
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_const_large() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = vec![0x42];
        bytecode.extend_from_slice(&leb128_i64(0x100000001i64));
        bytecode.push(0xA7);
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_const_negative() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = vec![0x42];
        bytecode.extend_from_slice(&leb128_i64(-1i64));
        bytecode.push(0xA7);
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_add_with_carry() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        // 0xFFFFFFFF + 1 = 0x100000000 (tests carry into hi word)
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(0xFFFFFFFFi64));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(1i64));
        bytecode.push(0x7C); // i64.add
        bytecode.push(0x50); // i64.eqz -> 0 (not zero)
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_add_simple() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(10));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(32));
        bytecode.push(0x7C); // i64.add
        bytecode.push(0xA7); // i32.wrap_i64
        bytecode.push(0x1A); // drop
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_sub() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(100));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(58));
        bytecode.push(0x7D); // i64.sub
        bytecode.push(0xA7); bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_mul() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(6));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(7));
        bytecode.push(0x7E); // i64.mul
        bytecode.push(0xA7); bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_extend_i32_u() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let bytecode: Vec<u8> = vec![
            0x41, 0x2A, // i32.const 42
            0xAD,       // i64.extend_i32_u
            0xA7,       // i32.wrap_i64
            0x1A, 0x0B,
        ];
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_extend_i32_s() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let bytecode: Vec<u8> = vec![
            0x41, 0x7F, // i32.const -1
            0xAC,       // i64.extend_i32_s
            0x50,       // i64.eqz -> 0 (non-zero)
            0x1A, 0x0B,
        ];
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_store_load() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.extend_from_slice(&[0x41, 0xe4, 0x00]); // i32.const 100
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(0x0000000200000001i64));
        bytecode.extend_from_slice(&[0x37, 0x00, 0x00]); // i64.store
        bytecode.extend_from_slice(&[0x41, 0xe4, 0x00]); // i32.const 100
        bytecode.extend_from_slice(&[0x29, 0x00, 0x00]); // i64.load
        bytecode.push(0xA7); // i32.wrap_i64
        bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_bitwise() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(0xFF));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(0x0F));
        bytecode.push(0x83); // i64.and
        bytecode.push(0xA7); bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_shl() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(1));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(32));
        bytecode.push(0x86); // i64.shl
        bytecode.push(0x50); // i64.eqz -> 0
        bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_comparisons() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(10));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(20));
        bytecode.push(0x54); // i64.lt_u
        bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_clz() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(1));
        bytecode.push(0x79); // i64.clz
        bytecode.push(0xA7); bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_div_u() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(84));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(2));
        bytecode.push(0x80); // i64.div_u
        bytecode.push(0xA7); bytecode.push(0x1A); bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }

    #[test]
    fn test_i64_div_by_zero_traps() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        let mut bytecode: Vec<u8> = Vec::new();
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(42));
        bytecode.push(0x42); bytecode.extend_from_slice(&leb128_i64(0));
        bytecode.push(0x80); // i64.div_u
        bytecode.push(0x0B);
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 1, "should eject on div by zero");
        assert_eq!(results[0].ejection_reason, 7, "should trap (reason=7)");
    }

    #[test]
    fn test_i64_in_if_else_skip() {
        let _ = env_logger::try_init();
        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => { eprintln!("Skipping GPU test: {}", e); return; }
        };
        // Test that label-skipping correctly handles i64.const LEB128 in dead branch
        let mut bytecode: Vec<u8> = vec![
            0x41, 0x00, // i32.const 0 (false)
            0x04, 0x40, // if [void]
        ];
        bytecode.push(0x42); // i64.const (in dead branch)
        bytecode.extend_from_slice(&leb128_i64(99999i64));
        bytecode.push(0x1A); // drop
        bytecode.push(0x05); // else
        bytecode.push(0x01); // nop
        bytecode.push(0x0B); // end (if)
        bytecode.push(0x0B); // end (func)
        let input = build_input(1, 0, 100, 1_000_000, &bytecode, 0, 0);
        let results = gpu.execute_shard_raw(&input, 1).unwrap();
        assert_eq!(results[0].ejected, 0, "should not eject: reason={}", results[0].ejection_reason);
    }
}


#[cfg(test)]
mod tests_multi_contract {
    use super::*;
    use crate::pipeline::{build_shard_input_multi, ContractInfo};

    #[test]
    fn test_multi_contract_dispatch() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Contract B: simple contract that pushes 42 and returns
        // Bytecode: i32.const 42, end
        let contract_b_bytecode: Vec<u8> = vec![
            0x41, 0x2A, // i32.const 42
            0x0B,       // end
        ];

        // Contract A: writes target AlkaneId (block=2, tx=1) to memory,
        // then calls HOST_CALL targeting that contract.
        // After the call returns, the return value (42) is on the stack.
        // We drop it and end.
        let contract_a_bytecode: Vec<u8> = vec![
            // Store block=2 at memory offset 100 (little-endian u32)
            0x41, 0xe4, 0x00,       // i32.const 100
            0x41, 0x02,             // i32.const 2
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Store tx=1 at memory offset 116 (100 + 16)
            0x41, 0xf4, 0x00,       // i32.const 116
            0x41, 0x01,             // i32.const 1
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Push HOST_CALL args
            0x41, 0xe4, 0x00,       // i32.const 100  (cellpack_ptr)
            0x41, 0x00,             // i32.const 0    (incoming_alkanes_ptr)
            0x41, 0x00,             // i32.const 0    (checkpoint_ptr)
            0x41, 0x00,             // i32.const 0    (start_fuel lo)
            0x41, 0x00,             // i32.const 0    (start_fuel hi)
            0x10, 0x0F,             // call 15        (HOST_CALL)
            // After HOST_CALL returns, result is on stack
            0x1A,                   // drop (the return value)
            0x0B,                   // end
        ];

        // Build multi-contract shard
        let additional = vec![ContractInfo {
            alkane_id: (2, 1),  // block=2, tx=1
            bytecode: contract_b_bytecode,
            import_count: 0,
            entry_pc: 0,
            func_table: vec![],
            import_map: vec![],
            globals_packed: vec![],
            globals_count: 0,
            data_segments_packed: vec![],
            data_segments_count: 0,
        }];

        // Identity import map for 18 host functions
        let identity_map: Vec<u32> = (0..18).collect();
        let input = build_shard_input_multi(
            1,     // 1 message
            100,   // block_height
            1_000_000, // fuel
            &contract_a_bytecode,
            18,    // import_count (18 host functions)
            0,     // entry_pc
            &[],   // no func_table for primary
            &identity_map,
            &[], 0, &[], 0,  // no globals or data segments
            &additional,
            &[],   // no kv pairs
        );

        let results = gpu.execute_shard_raw(&input, 1).unwrap();

        assert_eq!(
            results[0].ejected, 0,
            "Contract A should complete without ejection. reason={}",
            results[0].ejection_reason
        );
        assert_eq!(
            results[0].success, 1,
            "Contract A should succeed"
        );
    }

    #[test]
    fn test_multi_contract_ejects_when_not_found() {
        let _ = env_logger::try_init();

        let gpu = match AlkanesGpu::new() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("Skipping GPU test (no adapter): {}", e);
                return;
            }
        };

        // Contract A calls a contract that is NOT in the multi-contract table
        let contract_a_bytecode: Vec<u8> = vec![
            // Store block=99 at memory offset 100
            0x41, 0xe4, 0x00,       // i32.const 100
            0x41, 0xe3, 0x00,       // i32.const 99
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Store tx=88 at memory offset 116
            0x41, 0xf4, 0x00,       // i32.const 116
            0x41, 0xd8, 0x00,       // i32.const 88
            0x36, 0x02, 0x00,       // i32.store align=4 offset=0
            // Push HOST_CALL args
            0x41, 0xe4, 0x00,       // i32.const 100  (cellpack_ptr)
            0x41, 0x00,             // i32.const 0    (incoming_alkanes_ptr)
            0x41, 0x00,             // i32.const 0    (checkpoint_ptr)
            0x41, 0x00,             // i32.const 0    (start_fuel lo)
            0x41, 0x00,             // i32.const 0    (start_fuel hi)
            0x10, 0x0F,             // call 15        (HOST_CALL)
            0x0B,                   // end
        ];

        // Build with NO additional contracts
        let identity_map2: Vec<u32> = (0..18).collect();
        let input = build_shard_input_multi(
            1, 100, 1_000_000,
            &contract_a_bytecode,
            18, 0, &[],
            &identity_map2,
            &[], 0, &[], 0,  // no globals or data segments
            &[],  // no additional contracts
            &[],
        );

        let results = gpu.execute_shard_raw(&input, 1).unwrap();

        assert_eq!(results[0].ejected, 1, "should be ejected");
        assert_eq!(results[0].ejection_reason, 5, "reason should be EXTCALL (5)");
        assert_eq!(results[0].return_data_len, 32, "should have 32-byte return data");

        // Verify extracted AlkaneId
        let block = u128::from_le_bytes(results[0].return_data[0..16].try_into().unwrap());
        let tx = u128::from_le_bytes(results[0].return_data[16..32].try_into().unwrap());
        assert_eq!(block, 99, "target block should be 99");
        assert_eq!(tx, 88, "target tx should be 88");
    }
}

