use protorune_support::balance_sheet::{BalanceSheet, RuneTransfer, ProtoruneRuneId};
use protorune_support::message::{MessageContext, MessageContextParcel};
use anyhow::{anyhow, Result};
use protorune_support::host::Host;
use metashrew_support::index_pointer::KeyValuePointer;

#[derive(Clone, Default)]
pub struct AlkaneMessageContext(());

impl<T: Host> MessageContext<T> for AlkaneMessageContext {
    fn protocol_tag() -> u128 {
        1
    }
    fn asset_protoburned_in_protocol(_: ProtoruneRuneId) -> bool {
        false
    }
    fn handle<P: KeyValuePointer + Default + Clone>(
        _parcel: &MessageContextParcel<T>,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<P>)> where T::Pointer: Default + Clone {
        // This is a placeholder. The actual implementation will be in the `alkanes` crate.
        Err(anyhow!("not implemented"))
    }
}