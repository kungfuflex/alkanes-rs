use crate::tables::RuneTable;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Transaction};
use metashrew_support::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;
use std::u128;
use metashrew_support::environment::RuntimeEnvironment;
use std::marker::PhantomData;

pub trait MessageContext<E: RuntimeEnvironment + Clone> {
    fn handle(
        parcel: &MessageContextParcel<E>,
        env: &mut E,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<E, AtomicPointer<E>>)>;
    fn protocol_tag() -> u128;
    fn asset_protoburned_in_protocol(id: ProtoruneRuneId, env: &mut E) -> bool {
        let table = RuneTable::<E>::for_protocol(Self::protocol_tag());
        if table
            .RUNE_ID_TO_ETCHING
            .select(&id.into())
            .get(env)
            .len()
            > 0
        {
            return true;
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct MessageContextParcel<E: RuntimeEnvironment + Clone> {
    pub atomic: AtomicPointer<E>,
    pub runes: Vec<RuneTransfer>,
    pub transaction: Transaction,
    pub block: Block,
    pub height: u64,
    pub pointer: u32,
    pub refund_pointer: u32,
    pub calldata: Vec<u8>,
    pub sheets: Box<BalanceSheet<E, AtomicPointer<E>>>,
    pub txindex: u32,
    pub vout: u32,
    pub runtime_balances: Box<BalanceSheet<E, AtomicPointer<E>>>,
    pub _phantom: PhantomData<E>,
}

pub trait ToBytes {
    fn try_to_bytes(&self) -> Result<Vec<u8>>;
}

impl ToBytes for OutPoint {
    fn try_to_bytes(&self) -> Result<Vec<u8>> {
        Ok(consensus_encode(self)?)
    }
}

impl<E: RuntimeEnvironment + Clone> Default for MessageContextParcel<E> {
    fn default() -> MessageContextParcel<E> {
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
            runtime_balances: Box::new(BalanceSheet::new_ptr_backed(AtomicPointer::default())),
            sheets: Box::new(BalanceSheet::new_ptr_backed(AtomicPointer::default())),
            _phantom: PhantomData,
        }
    }
}