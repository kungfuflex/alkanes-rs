pub fn get_bytes() -> Vec<u8> {
    include_bytes!("./alkanes_std_auth_token.wasm").to_vec()
}
