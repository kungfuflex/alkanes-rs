#[allow(long_running_const_eval)]
pub fn get_bytes() -> Vec<u8> { include_bytes!("./fr_btc_testnet.wasm").to_vec() }
