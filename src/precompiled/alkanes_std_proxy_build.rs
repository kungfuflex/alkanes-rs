pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_proxy.wasm").to_vec()
}
