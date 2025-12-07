pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./ftr_btc.wasm").to_vec()
}
