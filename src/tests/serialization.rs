use anyhow::{anyhow, Result};
use hex_lit::hex;
use metashrew_core::{println, stdio::stdout};
use protobuf::{Message, MessageField};
use protorune_support::proto::protorune::ProtorunesWalletRequest;
use std::fmt::Write;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
pub fn test_decode() -> Result<()> {
    println!("{:?}", ProtorunesWalletRequest::parse_from_bytes(&(&hex!("0a406263727431703335687775396a306132377a637a6c6468337a36686e796b637972386a3577766837307a706c796a68616e377a647036763577736a6a75716430")).to_vec()).unwrap());
    Ok(())
}
