use crate::{id::AlkaneId, parcel::AlkaneTransferParcel};
use anyhow::Result;
use metashrew_support::utils::{consume_sized_int, is_empty};
use std::io::Cursor;

#[derive(Clone, Default, Debug)]
pub struct Context {
    pub myself: AlkaneId,
    pub caller: AlkaneId,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub inputs: Vec<u128>,
}

impl Context {
    pub fn parse(v: &mut Cursor<Vec<u8>>) -> Result<Context> {
        let mut result = Context::default();
        result.myself = AlkaneId::parse(v)?;
        result.caller = AlkaneId::parse(v)?;
        result.incoming_alkanes = AlkaneTransferParcel::parse(v)?;
        while !is_empty(v) {
            result.inputs.push(consume_sized_int::<u128>(v)?);
        }
        Ok(result)
    }
}