use protobuf::{Message, MessageField};
use wasm_bindgen_test::*;
use hex_lit::{hex};
use protorune_support::proto::protorune::ProtorunesWalletRequest;
use anyhow::{Result, anyhow};
use metashrew::{println, stdio::{stdout}};
use std::fmt::{Write};

#[wasm_bindgen_test]
pub fn test_decode() -> Result<()> {
  println!("{:?}", ProtorunesWalletRequest::parse_from_bytes(&(&hex!("0a406263727431703335687775396a306132377a637a6c6468337a36686e796b637972386a3577766837307a706c796a68616e377a647036763577736a6a75716430")).to_vec()).unwrap());
  Ok(())
}
