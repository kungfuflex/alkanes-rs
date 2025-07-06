#![cfg_attr(target_arch = "spirv", no_std)]
#![cfg_attr(target_arch = "spirv", no_main, feature(register_attr), register_attr(spirv))]

#[cfg(target_arch = "spirv")]
use spirv_std::glam::UVec3;
#[cfg(target_arch = "spirv")]
use spirv_std::spirv;

#[cfg(not(target_arch = "spirv"))]
pub struct UVec3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[cfg(target_arch = "spirv")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Minimal test compute shader
#[cfg(target_arch = "spirv")]
#[spirv(compute(threads(1, 1, 1)))]
pub fn test_compute(
    #[spirv(global_invocation_id)] _global_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] data: &mut [u32],
) {
    if !data.is_empty() {
        data[0] = 42;
    }
}