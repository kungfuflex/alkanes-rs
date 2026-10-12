pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_diesel_v3_testnet.wasm").to_vec()
}
