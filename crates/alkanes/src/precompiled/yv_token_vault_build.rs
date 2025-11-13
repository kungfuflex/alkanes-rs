pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./yv_token_vault.wasm").to_vec()
}
