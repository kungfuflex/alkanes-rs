use crate::vm::instance::AlkanesInstance as AlkanesVm;
use crate::WasmHost;
use anyhow::Result;
use bitcoin::Transaction;
use ordinals::{Etching, Rune, RuneId};
use protorune_support::{
    balance_sheet::BalanceSheet,
    message::MessageContext,
};
use ordinals::runestone::message::Message;
use std::collections::{BTreeMap, HashMap};

use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::message::MessageContextParcel;
use metashrew_support::index_pointer::KeyValuePointer;

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