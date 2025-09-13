use anyhow::Result;
use serde_json::{Value};
use alkanes_std_orbital;

#[test]
fn abi() -> Result<()> {
    let abi_bytes = alkanes_std_orbital::get_abi();
    let abi_string = String::from_utf8(abi_bytes.clone())?;
    let abi_json: Value = serde_json::from_slice(&abi_bytes)?;
    assert_eq!(abi_json["contract"], "Orbital");
    Ok(())
}