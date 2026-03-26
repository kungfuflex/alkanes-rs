//! alkanes-gpu: GPU-accelerated WASM execution for the alkanes indexer.
//!
//! Uses wgpu + WGSL compute shaders to execute independent contract
//! messages in parallel on the GPU, with tiered fallback to CPU threads
//! and sequential wasmi execution.

pub mod device;
pub mod types;

use anyhow::Result;
use bytemuck::Zeroable;
use device::{ComputePipeline, GpuDevice};
use types::*;

/// The embedded WGSL shader source.
pub const SHADER_SOURCE: &str = include_str!("shader.wgsl");

/// Per-thread WASM memory size in u32 words (1 MB = 262144 words).
const WASM_MEMORY_WORDS_PER_THREAD: usize = 262144;
/// Per-thread execution state size in u32 words.
/// Must match THREAD_STATE_SIZE in shader.wgsl:
///   STACK_SIZE(512) + LOCALS_SIZE(256) + MAX_CALL_FRAMES(64)*4 + MAX_LABELS(128)*3 + SCALARS(12)
const THREAD_STATE_WORDS: usize = 512 + 256 + 64 * 4 + 128 * 3 + 12;

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
            10, // func_table_offset (right after header)
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

        let func_table_offset = 10 + bytecode_words.len() as u32;

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
        ];
        input.extend_from_slice(&bytecode_words);
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
