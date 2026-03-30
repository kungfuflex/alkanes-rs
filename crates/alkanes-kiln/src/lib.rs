//! alkanes-kiln: GPU execution runtime for alkanes contracts
//!
//! Manages the lifecycle of GPU-compiled contract kernels:
//! - Compilation cache (WASM bytecode hash -> compiled SPIR-V)
//! - Kernel dispatch and result collection
//! - K/V buffer management for host function emulation
//!
//! The `gpu` feature enables host-side LLVM compilation and wgpu dispatch.
//! Without it, only the data types and cache logic are available (safe for wasm32).

pub mod cache;

#[allow(unused_imports)]
use cache::CompilationCache;

/// Contract compilation cache key
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ContractId {
    pub block: u128,
    pub tx: u128,
}

/// Result of a GPU-executed contract
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Whether execution completed successfully
    pub success: bool,
    /// K/V writes produced by this execution
    pub kv_writes: Vec<(Vec<u8>, Vec<u8>)>,
    /// Return data
    pub return_data: Vec<u8>,
    /// Gas consumed
    pub gas_used: u64,
}

/// Execution parameters passed as a uniform buffer to the GPU kernel.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "gpu", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct ExecParams {
    pub height: u32,
    pub fuel: u32,
    pub message_count: u32,
    pub _padding: u32,
}

/// Per-thread linear memory size: 16 MB
#[allow(dead_code)]
const MEMORY_SIZE: usize = 16 * 1024 * 1024;

// --- GPU dispatch (native only) ---

#[cfg(all(not(target_arch = "wasm32"), feature = "gpu"))]
mod gpu {
    use super::*;
    use anyhow::{anyhow, Context, Result};
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// A compiled GPU kernel ready for dispatch.
    pub struct GpuKernel {
        pub pipeline: wgpu::ComputePipeline,
        pub bind_group_layout: wgpu::BindGroupLayout,
        /// WASM bytecode hash (cache key)
        pub wasm_hash: [u8; 32],
    }

    /// The kiln runtime: manages GPU device, compilation, caching, and dispatch.
    pub struct KilnRuntime {
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: wgpu::AdapterInfo,
        /// Compiled pipeline cache: wasm_hash -> GpuKernel
        kernels: HashMap<[u8; 32], GpuKernel>,
        /// SPIR-V bytecode cache (memory + disk)
        spirv_cache: CompilationCache,
        /// The LLVM compiler instance (reserved for future direct-module API)
        #[allow(dead_code)]
        compiler: alkanes_llvm::WasmToSpirv,
    }

    impl KilnRuntime {
        /// Initialize the GPU runtime.
        /// `cache_dir`: optional directory for persisting compiled SPIR-V.
        pub fn new(cache_dir: Option<PathBuf>) -> Result<Self> {
            let (device, queue, adapter_info) = pollster::block_on(Self::init_gpu())?;
            log::info!(
                "alkanes-kiln: GPU device ready — {} ({:?})",
                adapter_info.name,
                adapter_info.device_type,
            );
            Ok(Self {
                device,
                queue,
                adapter_info,
                kernels: HashMap::new(),
                spirv_cache: CompilationCache::new(cache_dir),
                compiler: alkanes_llvm::WasmToSpirv::new(),
            })
        }

        async fn init_gpu() -> Result<(wgpu::Device, wgpu::Queue, wgpu::AdapterInfo)> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12,
                ..Default::default()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .context("no suitable GPU adapter found")?;
            let info = adapter.get_info();
            let features = wgpu::Features::SPIRV_SHADER_PASSTHROUGH;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("alkanes-kiln"),
                        required_features: features,
                        required_limits: wgpu::Limits {
                            max_storage_buffer_binding_size: 1024 * 1024 * 1024,
                            max_buffer_size: 1024 * 1024 * 1024,
                            ..wgpu::Limits::default()
                        },
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .context("failed to create GPU device")?;
            Ok((device, queue, info))
        }

        /// Get adapter info string.
        pub fn adapter_name(&self) -> &str {
            &self.adapter_info.name
        }

        /// Compile a contract to SPIR-V (if not cached) and create the GPU kernel.
        /// Returns the WASM hash for subsequent `execute` calls.
        pub fn get_or_compile(
            &mut self,
            wasm_bytes: &[u8],
            alkane_id: (u128, u128),
        ) -> Result<[u8; 32]> {
            let hash = CompilationCache::hash_wasm(wasm_bytes);

            // Already have a live kernel?
            if self.kernels.contains_key(&hash) {
                return Ok(hash);
            }

            // Check SPIR-V cache (memory + disk)
            let spirv_bytes = if let Some(cached) = self.spirv_cache.get(&hash) {
                log::info!("alkanes-kiln: SPIR-V cache hit for {}", hex::encode(&hash[..8]));
                cached.clone()
            } else {
                // Compile WASM -> LLVM IR -> SPIR-V via compile_and_emit_spirv
                log::info!(
                    "alkanes-kiln: compiling contract ({},{}) to SPIR-V",
                    alkane_id.0, alkane_id.1,
                );
                let spirv = compile_wasm_to_spirv_bytes(wasm_bytes, alkane_id)?;
                self.spirv_cache.insert(wasm_bytes, spirv.clone());
                spirv
            };

            // Create the wgpu pipeline from SPIR-V
            let gpu_kernel = self.create_kernel_from_spirv(&spirv_bytes, &hash)?;
            self.kernels.insert(hash, gpu_kernel);
            Ok(hash)
        }

        /// Create a wgpu compute pipeline from SPIR-V bytes.
        ///
        /// The SPIR-V from llvm-spirv-18 is OpenCL flavor. wgpu expects
        /// Vulkan compute SPIR-V. If loading fails, the error is propagated
        /// so we can diagnose the flavor mismatch.
        fn create_kernel_from_spirv(
            &self,
            spirv_bytes: &[u8],
            wasm_hash: &[u8; 32],
        ) -> Result<GpuKernel> {
            // SPIR-V must be aligned to 4 bytes (u32 words)
            if spirv_bytes.len() % 4 != 0 {
                return Err(anyhow!("SPIR-V size {} is not aligned to 4 bytes", spirv_bytes.len()));
            }
            let spirv_words: &[u32] = bytemuck::cast_slice(spirv_bytes);

            // Validate SPIR-V magic number
            if spirv_words.is_empty() || spirv_words[0] != 0x07230203 {
                return Err(anyhow!(
                    "invalid SPIR-V magic: expected 0x07230203, got 0x{:08x}",
                    spirv_words.first().copied().unwrap_or(0)
                ));
            }

            log::info!(
                "alkanes-kiln: loading SPIR-V ({} words) into wgpu",
                spirv_words.len(),
            );

            // Try to create the shader module from SPIR-V.
            // This uses SPIRV_SHADER_PASSTHROUGH which bypasses naga validation
            // and passes the SPIR-V directly to the Vulkan driver.
            let shader_module = unsafe {
                self.device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
                    label: Some("alkanes-contract"),
                    source: Cow::Borrowed(spirv_words),
                })
            };

            // Bind group layout for the contract execution model:
            //   binding 0: WASM linear memory (storage, read-write)
            //   binding 1: K/V store data (storage, read-write)
            //   binding 2: Execution params (uniform)
            let storage_rw = wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            };

            let bind_group_layout =
                self.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("alkanes-kiln-bgl"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: storage_rw,
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: storage_rw,
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    });

            let pipeline_layout =
                self.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("alkanes-kiln-layout"),
                        bind_group_layouts: &[&bind_group_layout],
                        push_constant_ranges: &[],
                    });

            let pipeline =
                self.device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some("alkanes-kiln-pipeline"),
                        layout: Some(&pipeline_layout),
                        module: &shader_module,
                        entry_point: Some("__execute"),
                        compilation_options: Default::default(),
                        cache: None,
                    });

            Ok(GpuKernel {
                pipeline,
                bind_group_layout,
                wasm_hash: *wasm_hash,
            })
        }

        /// Execute a compiled contract on the GPU.
        ///
        /// - `wasm_hash`: key returned by `get_or_compile`
        /// - `memory`: initial WASM linear memory contents (will be padded to MEMORY_SIZE)
        /// - `kv_data`: serialized key-value pairs for host function emulation
        /// - `height`: block height for execution context
        pub fn execute(
            &self,
            wasm_hash: &[u8; 32],
            memory: &[u8],
            kv_data: &[u8],
            height: u32,
        ) -> Result<ExecutionResult> {
            let kernel = self.kernels.get(wasm_hash)
                .ok_or_else(|| anyhow!("no compiled kernel for hash {}", hex::encode(&wasm_hash[..8])))?;

            // Pad memory to full size
            let mut mem_buf = vec![0u8; MEMORY_SIZE];
            let copy_len = memory.len().min(MEMORY_SIZE);
            mem_buf[..copy_len].copy_from_slice(&memory[..copy_len]);

            // KV data — ensure at least 4 bytes
            let kv_buf = if kv_data.is_empty() {
                vec![0u8; 4]
            } else {
                kv_data.to_vec()
            };

            // Execution params
            let params = ExecParams {
                height,
                fuel: 1_000_000,
                message_count: 1,
                _padding: 0,
            };
            let params_bytes: &[u8] = bytemuck::bytes_of(&params);

            // Create GPU buffers
            use wgpu::util::DeviceExt;

            let memory_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("memory"),
                contents: &mem_buf,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

            let kv_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("kv_store"),
                contents: &kv_buf,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

            let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("exec_params"),
                contents: params_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("staging"),
                size: MEMORY_SIZE as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Bind group
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("alkanes-kiln-bg"),
                layout: &kernel.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: memory_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: kv_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

            // Dispatch
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("alkanes-kiln-encoder"),
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("alkanes-kiln-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&kernel.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }

            // Copy memory buffer to staging for readback
            encoder.copy_buffer_to_buffer(
                &memory_buffer, 0,
                &staging_buffer, 0,
                MEMORY_SIZE as u64,
            );
            self.queue.submit(std::iter::once(encoder.finish()));

            // Read back results
            let slice = staging_buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
            self.device.poll(wgpu::Maintain::Wait);
            rx.recv()
                .context("GPU readback channel closed")?
                .context("GPU readback failed")?;

            let data = slice.get_mapped_range();
            let result_memory = data.to_vec();
            drop(data);
            staging_buffer.unmap();

            // For now, return the full memory as return_data.
            // A real implementation would parse KV writes from the kv_buffer.
            Ok(ExecutionResult {
                success: true,
                kv_writes: Vec::new(),
                return_data: result_memory,
                gas_used: 0,
            })
        }
    }

    /// Standalone helper: compile WASM to SPIR-V bytes.
    /// Creates a fresh LLVM context and compiler for thread safety.
    pub fn compile_wasm_to_spirv_bytes(
        wasm_bytes: &[u8],
        alkane_id: (u128, u128),
    ) -> Result<Vec<u8>> {
        let compiler = alkanes_llvm::WasmToSpirv::new();
        compiler.compile_and_emit_spirv(wasm_bytes, alkane_id)
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "gpu"))]
pub use gpu::*;

#[cfg(test)]
mod tests {
    use super::*;
    use super::cache::CompilationCache;

    #[test]
    fn test_contract_id() {
        let id = ContractId { block: 2, tx: 0 };
        assert_eq!(id.block, 2);
    }

    #[test]
    fn test_exec_params_size() {
        assert_eq!(std::mem::size_of::<ExecParams>(), 16);
    }

    #[test]
    fn test_cache_integration() {
        let mut cache = CompilationCache::new(None);
        let wasm = b"test contract bytecode";
        let spirv = vec![0x03, 0x02, 0x23, 0x07, 0x00, 0x01];

        cache.insert(wasm, spirv.clone());
        let hash = CompilationCache::hash_wasm(wasm);
        assert_eq!(cache.get(&hash).unwrap(), &spirv);
    }
}
