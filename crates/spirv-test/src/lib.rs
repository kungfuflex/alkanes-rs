#![cfg_attr(target_arch = "spirv", no_std)]
#![deny(warnings)]

use spirv_std::spirv;

#[spirv(fragment)]
pub fn main_fs(output: &mut spirv_std::glam::Vec4) {
    *output = spirv_std::glam::Vec4::new(1.0, 0.0, 0.0, 1.0);
}

#[spirv(vertex)]
pub fn main_vs(
    #[spirv(vertex_index)] vert_id: i32,
    #[spirv(position, invariant)] out_pos: &mut spirv_std::glam::Vec4,
) {
    *out_pos = spirv_std::glam::Vec4::new(
        (vert_id - 1) as f32,
        ((vert_id & 1) * 2 - 1) as f32,
        0.0,
        1.0,
    );
}
