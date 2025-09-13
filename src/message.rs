use crate::vm::instance::AlkanesInstance as AlkanesVm;
use crate::WasmHost;
use anyhow::Result;
use bitcoin::Transaction;
use ordinals::{Etching, Rune, RuneId};
use protorune_support::{
    balance_sheet::BalanceSheet,
    message::{Message, MessageContext},
};
use std::collections::{BTreeMap, HashMap};

pub struct AlkaneMessageContext {}

impl MessageContext<WasmHost> for AlkaneMessageContext {
    fn new(
        host: &WasmHost,
        tx: &Transaction,
        tx_index: u32,
        block_height: u32,
        block_time: u32,
    ) -> Result<Message> {
        let mut vm = AlkanesVm::new();
        let mut parcel = vm.build_parcel(host, tx, tx_index, block_height, block_time)?;
        parcel = vm.execute(&parcel)?;
        let mut etchings: HashMap<RuneId, Etching> = HashMap::new();
        let mut runes: HashMap<RuneId, Rune> = HashMap::new();
        let mut balances: BalanceSheet = BalanceSheet::new();
        vm.read_state(&parcel, &mut etchings, &mut runes, &mut balances)?;
        Ok(Message {
            etchings,
            runes,
            balances,
            ..Default::default()
        })
    }
    fn protocol_tag() -> u128 {
        1
    }
}