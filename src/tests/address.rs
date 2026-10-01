use bitcoin::Script;
use hex_lit::hex;
use metashrew_core::{println, stdio::stdout};
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
        "bcrt1qzr9vhs60g6qlmk7x3dd7g3ja30wyts48sxuemv",
        to_address_str(&Script::from_bytes(&hex!(
            "001410cacbc34f4681fddbc68b5be4465d8bdc45c2a7"
        )))
        .unwrap()
    );
    // `saved` is an owned snapshot, so it is still valid after the
    // set_network() above replaced the configuration it was taken from.
    if let Some(saved) = saved {
        set_network(saved);
    }
}
