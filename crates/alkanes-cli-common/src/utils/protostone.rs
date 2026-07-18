use crate::Vec;
use anyhow::Result;
use protorune_support::{
    protostone::{split_bytes, Protostone},
    utils::encode_varint_list,
};

pub trait Protostones {
    fn encipher(&self) -> Result<Vec<u128>>;
}

impl Protostones for Vec<Protostone> {
    fn encipher(&self) -> Result<Vec<u128>> {
        let mut values = Vec::<u128>::new();
        for stone in self {
            values.push(stone.protocol_tag);
            let varints = stone.to_integers()?;
            values.push(varints.len() as u128);
            values.extend(&varints);
        }
        Ok(split_bytes(&encode_varint_list(&values)))
    }
}