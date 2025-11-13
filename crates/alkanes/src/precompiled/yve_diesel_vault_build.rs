pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./yve_diesel_vault.wasm").to_vec()
}
