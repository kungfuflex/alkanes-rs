use crate::id::AlkaneId;
use anyhow::Result;
use metashrew_support::utils::consume_sized_int;
use protorune_support::utils::encode_varint_list;
use std::io::Cursor;

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cellpack {
    pub target: AlkaneId,
    pub inputs: Vec<u128>,
}

impl Cellpack {
    pub fn parse(cursor: &mut Cursor<Vec<u8>>) -> Result<Cellpack> {
        let target = AlkaneId::parse(cursor)?;
        let mut result = Cellpack::default();
        result.target = target;
        loop {
            match consume_sized_int::<u128>(cursor) {
                Ok(v) => result.inputs.push(v),
                Err(_) => {
                    break;
                }
            }
        }
        Ok(result)
    }
    pub fn to_vec(&self) -> Vec<u128> {
        let mut values = Vec::<u128>::with_capacity(self.inputs.len() + 2);
        values.push(self.target.block);
        values.push(self.target.tx);
        values.extend(&self.inputs);
        values
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.to_vec()
            .into_iter()
            .map(|v| (&v.to_le_bytes()).to_vec())
            .flatten()
            .collect::<Vec<u8>>()
    }

    pub fn encipher(&self) -> Vec<u8> {
        // leb encode the list
        return encode_varint_list(&self.to_vec());
    }

    // non LEB encipher if we ever need it
    // pub fn encipher(&self) -> Vec<u8> {
    //     let mut values = Vec::<u8>::new();
    //     values.extend(self.target.block.to_le_bytes());
    //     values.extend(self.target.tx.to_le_bytes());
    //     let inputs_le: Vec<u8> = self
    //         .inputs
    //         .iter()
    //         .flat_map(|&num| num.to_le_bytes()) // Convert each u128 to bytes
    //         .collect();
    //     values.extend(inputs_le);
    //     // leb encode the list
    //     return values;
    // }
}

impl TryFrom<Vec<u128>> for Cellpack {
    type Error = anyhow::Error;
    fn try_from(v: Vec<u128>) -> std::result::Result<Cellpack, Self::Error> {
        // Length guard added 2026-06-25 (audit candidate Ceyz.sats finding F1):
        // a protostone message that decodes to fewer than 2 varints (e.g. a
        // 15-byte continued-LEB payload [0x80;14, 0x01]) would otherwise reach
        // `&v[0..2]` below and PANIC with "range out of bounds" — the slice
        // expression is evaluated before the `try_from`'s `?`, so the panic
        // fires before any error can propagate. Combined with metashrew's
        // panic-stops-block behavior, a single ~37-byte mineable OP_RETURN
        // would halt every alkanes indexer network-wide until a patch +
        // reindex. The dispatcher in `crates/alkanes/src/message.rs` already
        // treats an Err from this `try_from` as "skip protostone, refund,
        // continue" — no behavior change for valid txs.
        if v.len() < 2 {
            return Err(anyhow::anyhow!(
                "cellpack: need at least 2 varints (target.block + target.tx), got {}",
                v.len()
            ));
        }
        Ok(Cellpack {
            target: <[u128; 2] as TryFrom<&[u128]>>::try_from(&v[0..2])?.into(),
            inputs: (&v[2..]).to_vec(),
        })
    }
}
