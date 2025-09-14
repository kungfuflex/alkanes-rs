use crate::message::MessageContext;
use crate::tables::RuneTable;
#[allow(unused_imports)]
use metashrew_core::{
    flush, input, println,
    stdio::{stdout, Write},
};
use crate::balance_sheet::ProtoruneRuneId;

use crate::host::Host;

pub fn index_unique_protorunes<H: Host, T: MessageContext<H>>(
    host: &H,
    height: u64,
    assets: Vec<ProtoruneRuneId>,
) -> Result<(), anyhow::Error> {
    let rune_table = RuneTable::for_protocol(T::protocol_tag());
    for v in assets.into_iter().map(|v| -> Vec<u8> { v.into() }) {
        host.index_protorune(&v, height, &rune_table)?;
    }
    Ok(())
}