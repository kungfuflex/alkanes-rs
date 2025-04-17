use crate::{id::AlkaneId, parcel::AlkaneTransferParcel};
use anyhow::Result;
use metashrew_support::utils::consume_sized_int;
use metashrew_support::utils::is_empty;
use std::io::Cursor;

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Context {
    pub myself: AlkaneId,
    pub caller: AlkaneId,
    pub vout: u32,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub inputs: Vec<u128>,
}

fn serialize_context(context: &Context) -> Vec<u8> {
    let mut result = vec![
        context.myself.block,
        context.myself.tx,
        context.caller.block,
        context.caller.tx,
        context.vout as u128,
        context.incoming_alkanes.0.len() as u128,
    ];
    let mut incoming_alkanes = context
        .incoming_alkanes
        .0
        .clone()
        .into_iter()
        .map(|v| vec![v.id.block, v.id.tx, v.value])
        .flatten()
        .collect::<Vec<u128>>();
    result.extend(&incoming_alkanes);
    result.extend(&context.inputs);
    result
        .into_iter()
        .map(|v| v.to_le_bytes().to_vec())
        .flatten()
        .collect::<Vec<u8>>()
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
        serialize_context(self)
    }
}
