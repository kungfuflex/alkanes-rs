use crate::tables::{traces, traces_by_height};
use alkanes_support::proto;
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::OutPoint;

use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use prost::Message;

use std::sync::Arc;


use metashrew_core::environment::MetashrewEnvironment;

pub fn save_trace(
    outpoint: &OutPoint,
    height: u64,
    trace: Trace,
    env: &mut MetashrewEnvironment,
) -> Result<()> {
    let buffer: Vec<u8> = consensus_encode::<OutPoint>(outpoint)?;
    traces().select(&buffer).set(
        env,
        Arc::new(<Trace as Into<proto::alkanes::AlkanesTrace>>::into(trace).encode_to_vec()),
    );
    traces_by_height()
        .select_value(height)
        .append(env, Arc::new(buffer.clone()));
    Ok(())
}