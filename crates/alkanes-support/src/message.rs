use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use ordinals::runestone::message::Message;
use protorune_support::host::Host;

#[derive(Clone, Default)]
pub struct AlkaneMessageContext(());

pub trait MessageContext<T: Host> {
    fn new(
        host: &T,
        tx: &Transaction,
        tx_index: u32,
        block_height: u32,
        block_time: u32,
    ) -> Result<Message>;
    fn protocol_tag() -> u128;
}