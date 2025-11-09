pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./yv_fr_btc_vault.wasm").to_vec()
}
