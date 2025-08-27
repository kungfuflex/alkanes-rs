pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./fr_btc_signet.wasm").to_vec()
}
