#[cfg(not(test))]
use crate::WasmHost;
#[cfg(test)]
use alkanes::WasmHost;
use anyhow::Result;
use protorune_support::{
    balance_sheet::BalanceSheet,
    message::MessageContext,
};

use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::message::MessageContextParcel;

pub struct AlkaneMessageContext {}

impl MessageContext<WasmHost> for AlkaneMessageContext {
    fn handle(
        _parcel: &MessageContextParcel<WasmHost>,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<WasmHost>)>
    {
        todo!()
    }
    fn protocol_tag() -> u128 {
        1
    }
    fn asset_protoburned_in_protocol(_id: ProtoruneRuneId) -> bool {
        false
    }
}