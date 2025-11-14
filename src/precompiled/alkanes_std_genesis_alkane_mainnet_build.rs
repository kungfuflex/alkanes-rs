pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_genesis_alkane_mainnet.wasm").to_vec()
}