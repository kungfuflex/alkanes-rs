pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./gauge_contract.wasm").to_vec()
}
