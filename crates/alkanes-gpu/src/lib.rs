//! alkanes-gpu: GPU-accelerated WASM execution for the alkanes indexer.
//!
//! Uses wgpu + WGSL compute shaders to execute independent contract
//! messages in parallel on the GPU, with tiered fallback to CPU threads
//! and sequential wasmi execution.

pub mod device;
pub mod types;

use anyhow::Result;
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
        input_buffer: &[u8],
        message_count: usize,
    ) -> Result<Vec<GpuMessageResult>> {
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

        let results: &[GpuMessageResult] =
            bytemuck::cast_slice(&output_bytes[..output_size]);

        Ok(results.to_vec())
    }

    /// Simple shard execution with a header only (for testing).
    pub fn execute_shard(
        &self,
        header: &ShardHeader,
        _messages: &[GpuMessageInput],
        _kv_pairs: &[GpuKvPair],
    ) -> Result<Vec<GpuMessageResult>> {
        let input_bytes = bytemuck::bytes_of(header);
        self.execute_shard_raw(input_bytes, header.message_count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Build a minimal input buffer:
        // Header: message_count=4, kv_count=0, height=100, fuel=1000000, 0,
        //         bytecode_len=0, import_count=0, entry_pc=0
        let header: [u32; 8] = [4, 0, 100, 1_000_000, 0, 0, 0, 0];
        let input_bytes = bytemuck::cast_slice(&header);

        let results = gpu.execute_shard_raw(input_bytes, 4).unwrap();
        assert_eq!(results.len(), 4);

        for (i, r) in results.iter().enumerate() {
            // With bytecode_len=0, the interpreter loop doesn't execute,
            // so each thread writes success with remaining fuel
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
        // 0x41 = i32.const, 0x2A = 42 (LEB128), 0x0B = end
        let bytecode: Vec<u8> = vec![0x41, 0x2A, 0x0B];
        let bytecode_len = bytecode.len() as u32;

        // Pad bytecode to u32 boundary
        let mut bytecode_padded = bytecode.clone();
        while bytecode_padded.len() % 4 != 0 {
            bytecode_padded.push(0);
        }

        // Build input buffer
        let mut input: Vec<u32> = vec![
            1,              // message_count
            0,              // kv_count
            100,            // block_height
            1_000_000,      // base_fuel_lo
            0,              // base_fuel_hi
            bytecode_len,   // bytecode_len (bytes)
            0,              // import_count
            0,              // entry_pc (start at byte 0)
        ];
        // Append bytecode as u32 words
        let bytecode_words: &[u32] = bytemuck::cast_slice(&bytecode_padded);
        input.extend_from_slice(bytecode_words);

        let input_bytes: &[u8] = bytemuck::cast_slice(&input);
        let results = gpu.execute_shard_raw(input_bytes, 1).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].success, 1, "should succeed");
        assert_eq!(results[0].ejected, 0, "should not eject");
        // Fuel should have been consumed (at least 2 instructions: i32.const + end)
        assert!(
            results[0].gas_used_lo < 1_000_000,
            "fuel should be consumed: remaining={}",
            results[0].gas_used_lo
        );
    }
}
