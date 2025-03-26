use crate::id::AlkaneId;
use anyhow::{anyhow, Result};
use protobuf::MessageField;

pub fn field_or_default<T: Into<RT>, RT: Default>(v: MessageField<T>) -> RT {
    v.into_option()
        .ok_or("")
        .and_then(|v| Ok(<T as Into<RT>>::into(v)))
        .unwrap_or_else(|_| <RT as Default>::default())
}

pub fn shift<T>(v: &mut Vec<T>) -> Option<T> {
    if v.is_empty() {
        None
    } else {
        Some(v.remove(0))
    }
}

pub fn shift_or_err(v: &mut Vec<u128>) -> Result<u128> {
    shift(v)
        .ok_or("")
        .map_err(|_| anyhow!("expected u128 value in list but list is exhausted"))
}

pub fn shift_id(v: &mut Vec<u128>) -> Option<AlkaneId> {
    let block = shift(v)?;
    let tx = shift(v)?;
    Some(AlkaneId { block, tx })
}

pub fn shift_id_or_err(v: &mut Vec<u128>) -> Result<AlkaneId> {
    shift_id(v)
        .ok_or("")
        .map_err(|_| anyhow!("failed to shift AlkaneId from list"))
}

pub fn shift_as_long(v: &mut Vec<u128>) -> Option<u64> {
    Some(shift(v)?.try_into().ok()?)
}

pub fn shift_as_long_or_err(v: &mut Vec<u128>) -> Result<u64> {
    shift_as_long(v)
        .ok_or("")
        .map_err(|_| anyhow!("failed to shift u64 from list"))
}

pub fn overflow_error<T>(v: Option<T>) -> Result<T> {
    v.ok_or("").map_err(|_| anyhow!("overflow error"))
}

/// A macro that captures the expression and passes it to overflow_error
#[macro_export]
macro_rules! checked_expr {
    ($expr:expr) => {
        match $expr {
            Some(val) => Ok(val),
            None => Err(anyhow::anyhow!(concat!(
                "Overflow error in expression: ",
                stringify!($expr)
            ))),
        }
    };
}

pub fn shift_bytes32(v: &mut Vec<u128>) -> Option<Vec<u8>> {
    Some(
        (&[
            shift_as_long(v)?,
            shift_as_long(v)?,
            shift_as_long(v)?,
            shift_as_long(v)?,
        ])
            .to_vec()
            .into_iter()
            .rev()
            .fold(Vec::<u8>::new(), |mut r, v| {
                r.extend(&v.to_be_bytes());
                r
            }),
    )
}

pub fn shift_bytes32_or_err(v: &mut Vec<u128>) -> Result<Vec<u8>> {
    shift_bytes32(v)
        .ok_or("")
        .map_err(|_| anyhow!("failed to shift bytes32 from list"))
}
