pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_diesel_v3_regtest.wasm").to_vec()
}
