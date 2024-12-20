use crate::message::{MessageContext};
use metashrew_support::index_pointer::{KeyValuePointer};
use metashrew::index_pointer::{AtomicPointer};
use protorune_support::balance_sheet::{ProtoruneRuneId};
use crate::tables::{RuneTable};
use std::sync::{Arc};

pub fn index_unique_protorunes<T: MessageContext>(atomic: &mut AtomicPointer, height: u64, assets: Vec<ProtoruneRuneId>) {
      let rune_table = RuneTable::for_protocol(T::protocol_tag());
      let table = atomic.derive(&rune_table.HEIGHT_TO_RUNE_ID);
      let seen_table = atomic.derive(&rune_table.RUNE_ID_TO_INITIALIZED);
      assets.into_iter().map(|v| -> Vec<u8> { v.into() }).for_each(|v| {
        if seen_table.select(&v).get().as_ref().len() == 0 {
          seen_table.select(&v).set(Arc::new(vec![0x01]));
          table.select(&v).select_value::<u64>(height).append(Arc::new(v));
        }
      });
}
