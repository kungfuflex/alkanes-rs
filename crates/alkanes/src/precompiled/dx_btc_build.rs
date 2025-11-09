pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./dx_btc.wasm").to_vec()
}
