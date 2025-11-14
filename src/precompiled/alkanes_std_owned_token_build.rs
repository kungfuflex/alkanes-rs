pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_owned_token.wasm").to_vec()
}
