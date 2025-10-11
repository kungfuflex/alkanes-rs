use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::utils::overflow_error;
use alkanes_support::witness::find_witness_payload;
use alkanes_support::{context::Context, parcel::AlkaneTransfer};
use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_decode;
use std::sync::Arc;

fn name_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/name")
}

fn symbol_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/symbol")
}

pub fn trim(v: u128) -> String {
    String::from_utf8(
        v.to_le_bytes()
            .into_iter()
            .fold(Vec::<u8>::new(), |mut r, v| {
                if v != 0 {
                    r.push(v)
                }
                r
            }),
    )
    .unwrap()
}



pub trait MintableToken: AlkaneResponder {
    fn name(&mut self) -> String {
        String::from_utf8(self.name_pointer().get(self.env()).as_ref().clone())
            .expect("name not saved as utf-8, did this deployment revert?")
    }
    fn symbol(&mut self) -> String {
        String::from_utf8(self.symbol_pointer().get(self.env()).as_ref().clone())
            .expect("symbol not saved as utf-8, did this deployment revert?")
    }
    fn set_name_and_symbol(&mut self, name: u128, symbol: u128) {
        self.set_string_field_from_u128(self.name_pointer(), name);
        self.set_string_field_from_u128(self.symbol_pointer(), symbol);
    }
    fn set_name_and_symbol_str(&mut self, name: String, symbol: String) {
        self.set_string_field(self.name_pointer(), name);
        self.set_string_field(self.symbol_pointer(), symbol);
    }
    fn name_pointer(&self) -> StoragePointer {
        name_pointer()
    }
    fn symbol_pointer(&self) -> StoragePointer {
        symbol_pointer()
    }
    fn set_string_field_from_u128(&mut self, mut pointer: StoragePointer, v: u128) {
        pointer.set(self.env(), Arc::new(trim(v).as_bytes().to_vec()));
    }
    fn set_string_field(&mut self, mut pointer: StoragePointer, v: String) {
        pointer.set(self.env(), Arc::new(v.as_bytes().to_vec()));
    }
    fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }
    fn total_supply(&mut self) -> u128 {
        self.total_supply_pointer().get_value::<u128>(self.env())
    }
    fn set_total_supply(&mut self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(self.env(), v);
    }
    fn increase_total_supply(&mut self, v: u128) -> Result<()> {
        let total_supply = self.total_supply();
        self.set_total_supply(overflow_error(total_supply.checked_add(v))?);
        Ok(())
    }
    fn decrease_total_supply(&mut self, v: u128) -> Result<()> {
        let total_supply = self.total_supply();
        self.set_total_supply(overflow_error(total_supply.checked_sub(v))?);
        Ok(())
    }
    fn mint(&mut self, context: &Context, value: u128) -> Result<AlkaneTransfer> {
        self.increase_total_supply(value)?;
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }
    fn data_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/data")
    }
    fn data(&mut self) -> Vec<u8> {
        self.data_pointer().get(self.env()).as_ref().clone()
    }
    fn set_data(&mut self) -> Result<()> {
        let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(self.transaction()))?;
        self.data_pointer()
            .set(self.env(), Arc::new(find_witness_payload(&tx, 0).ok_or_else(|| {
                anyhow!("mintable token: witness envelope at index 0 does not contain data")
            })?));
        Ok(())
    }
}