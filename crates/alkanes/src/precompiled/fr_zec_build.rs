pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./fr_zec.wasm").to_vec()
}
