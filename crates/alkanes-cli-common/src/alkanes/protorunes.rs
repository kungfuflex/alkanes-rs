//! Data structures for protorunes commands
use crate::index_pointer::StubPointer;
use bitcoin::{TxOut, OutPoint};
use crate::alkanes::balance_sheet::BalanceSheetOperations;
use serde::{Deserialize, Serialize};

/// Represents the response for a single outpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoruneOutpointResponse {
    pub output: TxOut,
    pub outpoint: OutPoint,
    pub balance_sheet: crate::alkanes::balance_sheet::BalanceSheet<StubPointer, ()>,
}

impl Default for ProtoruneOutpointResponse {
    fn default() -> Self {
        Self {
            output: TxOut { value: bitcoin::Amount::from_sat(0), script_pubkey: Default::default() },
            outpoint: OutPoint::null(),
            balance_sheet: crate::alkanes::balance_sheet::BalanceSheet::new(),
        }
    }
}

/// Represents the response for a wallet's protorunes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtoruneWalletResponse {
    pub balances: Vec<ProtoruneOutpointResponse>,
}
use crate::{Result, alkanes::protoburn::Protoburn};
use crate::{
    alkanes::protostone::{Protostone},
};

pub trait Protostones {
    fn burns(&self) -> Result<Vec<Protoburn>>;
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
        // Note: This is a simplified version of split_bytes.
        // A full implementation would need to handle byte packing more carefully.
        Ok(values)
    }
    fn burns(&self) -> Result<Vec<Protoburn>> {
        Ok(self
            .into_iter()
            .filter(|stone| stone.burn.is_some())
            .map(|stone| Protoburn {
                tag: stone.burn.map(|v| v as u128),
                pointer: stone.pointer,
                from: stone.from.map(|v| vec![v]),
            })
            .collect())
    }
}