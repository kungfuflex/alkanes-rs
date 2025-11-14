pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_upgradeable.wasm").to_vec()
}
