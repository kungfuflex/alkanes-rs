#[allow(long_running_const_eval)]
pub fn get_bytes() -> Vec<u8> { include_bytes!("./fr_btc.wasm").to_vec() }
