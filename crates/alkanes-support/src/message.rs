use anyhow::Result;
pub use protorune_support::{
    balance_sheet::{BalanceSheet, ProtoruneRuneId},
    host::Host,
    message::{MessageContext, MessageContextParcel},
    rune_transfer::RuneTransfer,
};

#[derive(Clone, Default)]
pub struct AlkaneMessageContext(());

/*
 * Chadson's Journal:
 *
 * The `handle` function signature in this file was out of sync with the
 * `MessageContext` trait definition in `protorune-support`. The original
 * implementation had an unnecessary generic parameter `P` and was missing
 * the `Host` trait bound on the `BalanceSheet` return type.
 *
 * I have corrected the signature to match the trait, removing the generic `P`
 * and using the `T` generic from the `impl` block, which already has the
 * `Host` trait bound. This resolves the compilation errors related to the
 * trait implementation mismatch.
 */
impl<T: Host> MessageContext<T> for AlkaneMessageContext
where
    T: Default + Clone,
{
    fn handle(
        _parcel: &MessageContextParcel<T>,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<T>)> {
        todo!()
    }
    fn protocol_tag() -> u128 {
        12
    }
    fn asset_protoburned_in_protocol(_id: ProtoruneRuneId) -> bool {
        todo!()
    }
}