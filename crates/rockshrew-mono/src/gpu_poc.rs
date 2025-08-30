// gpu_poc.rs
// This file contains the proof-of-concept logic for running a simple compute shader.
// It's part of Phase A of the GPU Parallelization Plan.

use wgpu::util::DeviceExt;

pub async fn run_gpu_poc() -> Result<Vec<u32>, anyhow::Error> {
    // 1. Initialize WGPU
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to find a wgpu adapter."))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GPU PoC Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await?;

    // 2. Load the shader
    let shader_source = include_str!("gpu_poc.wgsl");
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("GPU PoC Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // 3. Create pipeline
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("GPU PoC Pipeline"),
        layout: None,
        module: &shader_module,
        entry_point: "main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    });

    // 4. Create data buffers
    let initial_data: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let buffer_size = (initial_data.len() * std::mem::size_of::<u32>()) as u64;

    let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Storage Buffer"),
        contents: bytemuck::cast_slice(&initial_data),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
    });

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // 5. Create bind group
    let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("GPU PoC Bind Group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage_buffer.as_entire_binding(),
        }],
    });

    // 6. Dispatch the compute shader
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("GPU PoC Command Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("GPU PoC Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(initial_data.len() as u32, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, buffer_size);

    queue.submit(Some(encoder.finish()));

    // 7. Read back the results
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    device.poll(wgpu::Maintain::Wait);

    if let Some(Ok(())) = receiver.receive().await {
        let data = buffer_slice.get_mapped_range();
        let result: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();
        Ok(result)
    } else {
        Err(anyhow::anyhow!("Failed to read back GPU results."))
    }
}