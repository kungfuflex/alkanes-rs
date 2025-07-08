//! alkanes-gpu-shader: Minimal SPIR-V compute shader for alkanes message processing
//!
//! This crate provides a minimal GPU shader that compiles to SPIR-V without complex dependencies.

#![no_std]
#![cfg_attr(target_arch = "spirv", no_main)]

// Only import spirv-std when compiling for SPIR-V target
#[cfg(target_arch = "spirv")]
use spirv_std::spirv;
#[cfg(target_arch = "spirv")]
use spirv_std::glam::UVec3;

/// GPU shader entry point for alkanes message processing
#[cfg(target_arch = "spirv")]
#[spirv(compute(threads(64)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] global_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] input_data: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] output_data: &mut [u32],
) {
    let thread_id = global_id.x;
    
    // Simple operation: copy input to output with thread_id offset
    if (thread_id as usize) < input_data.len() && (thread_id as usize) < output_data.len() {
        output_data[thread_id as usize] = input_data[thread_id as usize] + thread_id;
    }
}

// Entry point for non-SPIR-V targets (for testing compilation)
#[cfg(not(target_arch = "spirv"))]
fn main() {
    println!("alkanes-gpu-shader compiled successfully for non-SPIR-V target");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compilation() {
        // Simple test to verify the crate compiles
        assert!(true);
    }
}
