use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use alkanes_support::proto;
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::OutPoint;
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use prost::Message;

use std::sync::Arc;


pub fn save_trace<E: RuntimeEnvironment>(
    outpoint: &OutPoint,
    height: u64,
    trace: Trace,
    env: &mut E,
) -> Result<()> {
    let buffer: Vec<u8> = consensus_encode::<OutPoint>(outpoint)?;
    TRACES.select(&buffer).set(
        &<Trace as Into<proto::alkanes::AlkanesTrace>>::into(trace).encode_to_vec(),
        env,
    );
    TRACES_BY_HEIGHT.select_value(height).append(&buffer, env);
    Ok(())
}