// This is a simple compute shader that multiplies each element of a buffer by 2.

// The input and output buffers are bound to the shader.
@group(0) @binding(0) var<storage, read_write> data: array<u32>;

// The entry point for the compute shader.
// `builtin(global_invocation_id)` provides the unique ID for this shader invocation.
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    data[index] = data[index] * 2;
}