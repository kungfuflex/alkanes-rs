pub const WASM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/alkanes/wasm32-unknown-unknown/release/alkanes_std_pixel.wasm"
));