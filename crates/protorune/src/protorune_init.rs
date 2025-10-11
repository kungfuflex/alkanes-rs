use crate::message::MessageContext;
use crate::tables::RuneTable;
use metashrew_support::index_pointer::AtomicPointer;
#[allow(unused_imports)]

use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::ProtoruneRuneId;
use std::sync::Arc;
use metashrew_support::environment::RuntimeEnvironment;

pub fn index_unique_protorunes<E: RuntimeEnvironment + Clone, T: MessageContext<E>>(
    atomic: &mut AtomicPointer<E>,
    height: u64,
    assets: Vec<ProtoruneRuneId>,
    env: &mut E,
) {
    let rune_table = RuneTable::<E>::for_protocol(T::protocol_tag());
    let table = atomic.derive(&rune_table.HEIGHT_TO_RUNE_ID);
    let seen_table = atomic.derive(&rune_table.RUNE_ID_TO_INITIALIZED);
    assets
        .into_iter()
        .map(|v| -> Vec<u8> { v.into() })
        .for_each(|v| {
            if seen_table.select(&v).get(env).as_ref().len() == 0 {
                seen_table.select(&v).set(env, Arc::new(vec![0x01]));
                table.select_value::<u64>(height).append(env, Arc::new(v));
            }
        });
}