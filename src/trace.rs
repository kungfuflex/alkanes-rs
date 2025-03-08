use crate::tables::{TRACES, TRACES_BY_HEIGHT, BLOCK_TRACES_CACHE};
use alkanes_support::proto;
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::OutPoint;
use bitcoin::hashes::Hash;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protobuf::{Message, MessageField};
use std::sync::Arc;
#[allow(unused_imports)]
use {
    metashrew::{println, stdio::stdout},
    std::fmt::Write,
};
use protorune::tables::RUNES;

pub fn save_trace(outpoint: &OutPoint, height: u64, trace: Trace) -> Result<()> {
    let buffer: Vec<u8> = consensus_encode::<OutPoint>(outpoint)?;
    
    // Convert trace to AlkanesTrace protobuf and save to TRACES
    let alkanes_trace = <Trace as Into<proto::alkanes::AlkanesTrace>>::into(trace);
    let trace_bytes = alkanes_trace.write_to_bytes()?;
    TRACES.select(&buffer).set(Arc::<Vec<u8>>::new(trace_bytes.clone()));
    
    // Add to TRACES_BY_HEIGHT
    TRACES_BY_HEIGHT
        .select_value(height)
        .append(Arc::new(buffer.clone()));
    
    // Update or create BlockTrace in cache
    update_block_trace_cache(outpoint, height, alkanes_trace, &buffer)?;
    
    Ok(())
}

// Helper function to update the block trace cache
fn update_block_trace_cache(outpoint: &OutPoint, height: u64, trace: proto::alkanes::AlkanesTrace, buffer: &Vec<u8>) -> Result<()> {
    let txid = outpoint.txid.as_byte_array().to_vec();
    let txindex: u32 = RUNES.TXID_TO_TXINDEX.select(&txid).get_value();
    
    // Create block event for this trace
    let block_event = proto::alkanes::AlkanesBlockEvent {
        txindex: txindex as u64,
        outpoint: MessageField::some(proto::alkanes::Outpoint {
            txid: outpoint.txid.as_byte_array().to_vec(),
            vout: outpoint.vout,
            ..Default::default()
        }),
        traces: MessageField::some(trace),
        ..Default::default()
    };
    
    // Get or create block trace
    let mut cache = BLOCK_TRACES_CACHE.write().unwrap();
    let mut block_trace = if let Some(cached_bytes) = cache.get(&height) {
        proto::alkanes::AlkanesBlockTraceEvent::parse_from_bytes(cached_bytes)?
    } else {
        proto::alkanes::AlkanesBlockTraceEvent::new()
    };
    
    // Add the event to the block trace
    block_trace.events.push(block_event);
    
    // Serialize and store updated BlockTrace
    let serialized = block_trace.write_to_bytes()?;
    cache.insert(height, serialized);
    
    Ok(())
}
