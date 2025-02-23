use bitcoin::Script;
use hex_lit::hex;
use metashrew::{println, stdio::stdout};
use metashrew_support::address::Payload;
use protorune_support::network::{get_network_option, set_network, to_address_str, NetworkParams};
use std::fmt::Write;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
pub fn test_address_generation() {
    let saved = get_network_option();
    set_network(NetworkParams {
        bech32_prefix: String::from("bcrt"),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });
    assert_eq!(
        "bcrt1pys2f8u8yx7nu08txn9kzrstrmlmpvfprdazz9se5qr5rgtuz8htsaz3chd",
        to_address_str(&Script::from_bytes(&hex!(
            "5120241493f0e437a7c79d66996c21c163dff61624236f4422c33400e8342f823dd7"
        )))
        .unwrap()
    );
    if saved.is_some() {
        set_network(saved.unwrap().clone());
    }
}
