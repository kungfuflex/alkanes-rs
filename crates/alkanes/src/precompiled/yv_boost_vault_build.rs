pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./yv_boost_vault.wasm").to_vec()
}
