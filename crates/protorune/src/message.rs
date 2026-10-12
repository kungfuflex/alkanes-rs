use crate::tables::RuneTable;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Transaction};
use metashrew_core::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_encode;
use std::sync::Arc;
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
    /// Shared, read-only. One `Arc` per indexed block (see [`shared_block`]):
    /// every message used to carry its own deep copy of the whole block, which
    /// made a block's indexing cost grow with (messages x block size).
    pub block: Arc<Block>,
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
            block: Arc::new(block.clone()),
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

thread_local! {
    static SHARED_BLOCK: std::cell::RefCell<Option<Arc<Block>>> = std::cell::RefCell::new(None);
}

/// Set for the duration of `Protorune::index_block`: the block being indexed,
/// shared by every message parcel built for it. Cleared on drop.
pub struct SharedBlockGuard(());

impl SharedBlockGuard {
    pub fn set(block: Arc<Block>) -> Self {
        SHARED_BLOCK.with(|c| *c.borrow_mut() = Some(block));
        SharedBlockGuard(())
    }
}

impl Drop for SharedBlockGuard {
    fn drop(&mut self) {
        SHARED_BLOCK.with(|c| *c.borrow_mut() = None);
    }
}

/// The shared `Arc` for `block` when it IS the block currently being indexed
/// (pointer identity, never block-hash equality: distinct blocks can share a
/// header in tests); otherwise a fresh `Arc` of a copy, as before.
pub fn shared_block(block: &Block) -> Arc<Block> {
    SHARED_BLOCK.with(|c| {
        if let Some(shared) = c.borrow().as_ref() {
            if std::ptr::eq(Arc::as_ptr(shared), block as *const Block) {
                return shared.clone();
            }
        }
        Arc::new(block.clone())
    })
}
