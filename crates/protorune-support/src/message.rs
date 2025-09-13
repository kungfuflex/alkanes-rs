use crate::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use crate::host::Host;
use metashrew_support::index_pointer::KeyValuePointer;
use crate::rune_transfer::RuneTransfer;
use anyhow::Result;
use bitcoin::{Block, Transaction};
use metashrew_core::index_pointer::AtomicPointer;
use std::u128;

pub trait MessageContext<T: Host> {
    fn handle<P: KeyValuePointer + Default + Clone>(parcel: &MessageContextParcel<T>) -> Result<(Vec<RuneTransfer>, BalanceSheet<P>)> where T::Pointer: Default + Clone;
    fn protocol_tag() -> u128;
    fn asset_protoburned_in_protocol(id: ProtoruneRuneId) -> bool;
}

#[derive(Clone, Debug)]
pub struct MessageContextParcel<'a, T: Host>
where
    T::Pointer: Default + Clone,
{
    pub host: &'a T,
    pub atomic: AtomicPointer,
    pub runes: Vec<RuneTransfer>,
    pub transaction: Transaction,
    pub block: Block,
    pub height: u64,
    pub pointer: u32,
    pub refund_pointer: u32,
    pub calldata: Vec<u8>,
    pub sheets: Box<BalanceSheet<T::Pointer>>,
    pub txindex: u32,
    pub vout: u32,
    pub runtime_balances: Box<BalanceSheet<T::Pointer>>,
}
