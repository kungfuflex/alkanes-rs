pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_genesis_alkane_upgraded_eoa_regtest.wasm").to_vec()
}
