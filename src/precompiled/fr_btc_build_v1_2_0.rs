pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./fr_btc_v1.2.0.wasm").to_vec()
}
