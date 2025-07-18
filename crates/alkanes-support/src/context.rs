use crate::{id::AlkaneId, parcel::AlkaneTransferParcel};
use anyhow::Result;
use metashrew_support::utils::consume_sized_int;
use metashrew_support::utils::is_empty;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Context {
    pub myself: AlkaneId,
    pub caller: AlkaneId,
    pub vout: u32,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub inputs: Vec<u128>,
}

impl Context {
    pub fn parse(v: &mut Cursor<Vec<u8>>) -> Result<Context> {
        let mut result = Context::default();
        result.myself = AlkaneId::parse(v)?;
        result.caller = AlkaneId::parse(v)?;
        result.vout = consume_sized_int::<u128>(v)?.try_into()?;
        result.incoming_alkanes = AlkaneTransferParcel::parse(v)?;
        while !is_empty(v) {
            result.inputs.push(consume_sized_int::<u128>(v)?);
        }
        Ok(result)
    }
    pub fn serialize(&self) -> Vec<u8> {
        vec![]
    }
}
