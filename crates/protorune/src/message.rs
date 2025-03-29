use crate::tables::RuneTable;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Transaction};
use metashrew::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;
use std::u128;

pub trait MessageContext {
    fn handle(
        parcel: &MessageContextParcel,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)>;
    fn protocol_tag() -> u128;
    fn asset_protoburned_in_protocol(id: ProtoruneRuneId) -> bool {
        let table = RuneTable::for_protocol(Self::protocol_tag());
        if table.RUNE_ID_TO_ETCHING.select(&id.into()).get().len() > 0 {
            return true;
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct MessageContextParcel {
    pub atomic: AtomicPointer,
    pub runes: Vec<RuneTransfer>,
    pub transaction: Transaction,
    pub block: Block,
    pub height: u64,
    pub pointer: u32,
    pub refund_pointer: u32,
    pub calldata: Vec<u8>,
    pub sheets: Box<BalanceSheet<AtomicPointer>>,
    pub txindex: u32,
    pub vout: u32,
    pub runtime_balances: Box<BalanceSheet<AtomicPointer>>,
}

pub trait ToBytes {
    fn try_to_bytes(&self) -> Result<Vec<u8>>;
}

impl ToBytes for OutPoint {
    fn try_to_bytes(&self) -> Result<Vec<u8>> {
        Ok(consensus_encode(self)?)
    }
}

impl Default for MessageContextParcel {
    fn default() -> MessageContextParcel {
        let block = bitcoin::constants::genesis_block(bitcoin::Network::Bitcoin);
        MessageContextParcel {
            atomic: AtomicPointer::default(),
            runes: Vec::<RuneTransfer>::default(),
            transaction: block.txdata[0].clone(),
            block: block.clone(),
            height: 0,
            pointer: 0,
            vout: 0,
            refund_pointer: 0,
            calldata: Vec::<u8>::default(),
            txindex: 0,
            runtime_balances: Box::new(BalanceSheet::default()),
            sheets: Box::new(BalanceSheet::default()),
        }
    }
}
