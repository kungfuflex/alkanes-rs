#![no_std]

use spirv_std::spirv;
use spirv_std::glam::{Vec3, Vec4};

#[spirv(vertex)]
pub fn main_vs(
    #[spirv(vertex_index)] vert_idx: i32,
    #[spirv(position, invariant)] out_pos: &mut Vec4,
) {
    // Create a simple triangle
    let uv = match vert_idx {
        0 => Vec3::new(-1.0, -1.0, 0.0),
        1 => Vec3::new(3.0, -1.0, 0.0),
        2 => Vec3::new(-1.0, 3.0, 0.0),
        _ => Vec3::new(0.0, 0.0, 0.0),
    };
    *out_pos = Vec4::new(uv.x, uv.y, uv.z, 1.0);
}

#[spirv(fragment)]
pub fn main_fs(output: &mut Vec4) {
    *output = Vec4::new(1.0, 0.0, 0.0, 1.0);
}