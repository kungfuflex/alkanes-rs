pub mod balance_sheet;
pub mod byte_utils;
pub mod constants;

pub mod proto {
    pub mod protorune {
        include!(concat!(env!("OUT_DIR"), "/protorune.rs"));
    }
}
pub mod protostone;
pub mod rune_transfer;
pub mod utils;

use anyhow;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, Txid};

impl TryFrom<proto::protorune::Outpoint> for OutPoint {
    type Error = anyhow::Error;
    fn try_from(outpoint: proto::protorune::Outpoint) -> Result<Self, Self::Error> {
        Ok(OutPoint {
            txid: Txid::from_slice(&outpoint.txid)?,
            vout: outpoint.vout,
        })
    }
}