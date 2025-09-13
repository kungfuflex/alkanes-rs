use crate::message::AlkaneMessageContext;
use anyhow::Result;
use bitcoin::{Block, Transaction, Witness};
use ordinals::{Artifact, Runestone};
use protorune_support::message::MessageContext;
use protorune_support::protostone::Protostone;
use protorune_support::utils::decode_varint_list;
use std::io::Cursor;

pub trait VirtualFuelBytes {
    fn vfsize(&self) -> u64;
}

impl VirtualFuelBytes for Transaction {
    fn vfsize(&self) -> u64 {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(&self) {
            if let Ok(protostones) = Protostone::from_runestone(runestone) {
                let cellpacks = protostones
                    .iter()
                    .filter_map(|v| {
                        if v.protocol_tag == AlkaneMessageContext::protocol_tag() {
                            decode_varint_list(&mut Cursor::new(v.message.clone()))
                                .and_then(|list| {
                                    if list.len() >= 2 {
                                        Ok(Some(list))
                                    } else {
                                        Ok(None)
                                    }
                                })
                                .unwrap_or_else(|_| None)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<Vec<u128>>>();
                if cellpacks.len() == 0 {
                    0
                } else if cellpacks
                    .iter()
                    .position(|v| {
                        <&[u128] as TryInto<[u128; 2]>>::try_into(&v[0..2]).unwrap()
                            == [1u128, 0u128]
                            || v[0] == 3u128
                    })
                    .is_some()
                {
                    let mut cloned = self.clone();
                    if cloned.input.len() > 0 {
                        cloned.input[0].witness = Witness::new();
                    }
                    cloned.vsize() as u64
                } else {
                    self.vsize() as u64
                }
            } else {
                0
            }
        } else {
            0
        }
    }
}

impl VirtualFuelBytes for Block {
    fn vfsize(&self) -> u64 {
        self.txdata.iter().fold(0u64, |r, v| r + v.vfsize())
    }
}