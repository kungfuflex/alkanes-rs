//! align-main: portable-crate replacement for develop's
//! `protorune_support::protostone::Protostones` trait.
//!
//! On develop, protorune-support carries a `Protostones` trait with an
//! `encipher()` method on `Vec<Protostone>`. On align-main protorune-support is
//! pinned byte-identical to v2.2.1-rc.3 (protected indexer closure), which does
//! not define that trait. This module re-implements the exact develop logic
//! locally so the portable CLI keeps its transaction-construction behavior
//! without modifying the protected crate. It relies only on public rc.3
//! protorune-support helpers (`split_bytes`, `Protostone::to_integers`,
//! `encode_varint_list`).

use crate::Result;
use protorune_support::protostone::{split_bytes, Protostone};
use protorune_support::utils::encode_varint_list;

/// Mirrors `protorune_support::protostone::Protostones` from develop.
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
