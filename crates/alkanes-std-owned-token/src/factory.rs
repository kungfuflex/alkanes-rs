use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::response::CallResponse;
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

pub struct ContextHandle(());
impl AlkaneResponder for ContextHandle {
    fn execute(&self) -> Result<CallResponse> {
        Ok(CallResponse::default())
    }
}

pub const CONTEXT: ContextHandle = ContextHandle(());

pub trait MintableToken {
    fn name(&self) -> String {
        String::from_utf8(self.name_pointer().get().as_ref().clone())
            .expect("name not saved as utf-8, did this deployment revert?")
    }
    fn symbol(&self) -> String {
        String::from_utf8(self.symbol_pointer().get().as_ref().clone())
            .expect("symbol not saved as utf-8, did this deployment revert?")
    }
    fn set_name_and_symbol(&self, name: u128, symbol: u128) {
        self.set_string_field_from_u128(self.name_pointer(), name);
        self.set_string_field_from_u128(self.symbol_pointer(), symbol);
    }
    fn set_name_and_symbol_str(&self, name: String, symbol: String) {
        self.set_string_field(self.name_pointer(), name);
        self.set_string_field(self.symbol_pointer(), symbol);
    }
    fn name_pointer(&self) -> StoragePointer {
        name_pointer()
    }
    fn symbol_pointer(&self) -> StoragePointer {
        symbol_pointer()
    }
    fn set_string_field_from_u128(&self, mut pointer: StoragePointer, v: u128) {
        pointer.set(Arc::new(trim(v).as_bytes().to_vec()));
    }
    fn set_string_field(&self, mut pointer: StoragePointer, v: String) {
        pointer.set(Arc::new(v.as_bytes().to_vec()));
    }
    fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }
    fn total_supply(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
    }
    fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }
    fn increase_total_supply(&self, v: u128) -> Result<()> {
        self.set_total_supply(overflow_error(self.total_supply().checked_add(v))?);
        Ok(())
    }
    fn mint(&self, context: &Context, value: u128) -> Result<AlkaneTransfer> {
        self.increase_total_supply(value)?;
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }
    fn data_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/data")
    }
    fn data(&self) -> Vec<u8> {
        self.data_pointer().get().as_ref().clone()
    }
    fn set_data(&self) -> Result<()> {
        let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(CONTEXT.transaction()))?;
        self.data_pointer()
            .set(Arc::new(find_witness_payload(&tx, 0).ok_or_else(|| {
                anyhow!("owned-token-factory: witness envelope at index 0 does not contain data")
            })?));
        Ok(())
    }
    fn observe_initialization(&self) -> Result<()> {
        let mut pointer = StoragePointer::from_keyword("/initialized");
        if pointer.get().len() == 0 {
            pointer.set_value::<u8>(0x01);
            Ok(())
        } else {
            Err(anyhow!("already initialized"))
        }
    }
}
