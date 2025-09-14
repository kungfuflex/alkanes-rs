use crate::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use crate::host::Host;
use crate::rune_transfer::RuneTransfer;
use anyhow::Result;
use bitcoin::{Block, Transaction};
use std::u128;

pub trait MessageContext<H: Host> {
    fn handle(parcel: &MessageContextParcel<H>) -> Result<(Vec<RuneTransfer>, BalanceSheet<H>)>;
    fn protocol_tag() -> u128;
    fn asset_protoburned_in_protocol(id: ProtoruneRuneId) -> bool;
}

use bitcoin::blockdata::block::Version as BlockVersion;
use bitcoin::blockdata::transaction::Version as TxVersion;
use bitcoin::hashes::Hash;
use bitcoin::{absolute, TxMerkleNode};

#[derive(Clone, Debug)]
pub struct MessageContextParcel<H: Host> {
    pub host: H,
    pub runes: Vec<RuneTransfer>,
    pub transaction: Transaction,
    pub block: Block,
    pub height: u64,
    pub pointer: u32,
    pub refund_pointer: u32,
    pub calldata: Vec<u8>,
    pub sheets: Box<BalanceSheet<H>>,
    pub txindex: u32,
    pub vout: u32,
    pub runtime_balances: Box<BalanceSheet<H>>,
}

impl<H: Host + Default> Default for MessageContextParcel<H> {
    fn default() -> Self {
        Self {
            host: H::default(),
            runes: Default::default(),
            transaction: Transaction {
                version: TxVersion(2),
                lock_time: absolute::LockTime::from_consensus(0),
                input: vec![],
                output: vec![],
            },
            block: Block {
                header: bitcoin::block::Header {
                    version: BlockVersion::from_consensus(0),
                    prev_blockhash: bitcoin::BlockHash::all_zeros(),
                    merkle_root: TxMerkleNode::all_zeros(),
                    time: 0,
                    bits: bitcoin::CompactTarget::from_consensus(0),
                    nonce: 0,
                },
                txdata: vec![],
            },
            height: 0,
            pointer: 0,
            refund_pointer: 0,
            calldata: vec![],
            sheets: Box::new(BalanceSheet::default()),
            txindex: 0,
            vout: 0,
            runtime_balances: Box::new(BalanceSheet::default()),
        }
    }
}
