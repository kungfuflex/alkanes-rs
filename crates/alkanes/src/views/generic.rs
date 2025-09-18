use super::types::{
    AlkaneIdToOutpointRequest, AlkaneIdToOutpointResponse, AlkaneInventoryRequest,
    AlkaneInventoryResponse, AlkaneStorageRequest, AlkaneStorageResponse, BytecodeRequest,
    BytecodeResponse, BlockRequest, BlockResponse, AlkaneTransfer
};
use crate::message::AlkaneMessageContext;
use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use crate::utils::{
    alkane_id_to_outpoint as alkane_id_to_outpoint_util,
    alkane_inventory_pointer, balance_pointer,
};
use crate::vm::runtime::AlkanesRuntimeContext;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use bitcoin::consensus::Decodable;
use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer, KeyValuePointer};
use metashrew_support::utils::consensus_encode;
use protorune::tables::RuneTable;
use std::io::Cursor;

pub fn alkanes_id_to_outpoint<E: RuntimeEnvironment + Clone + Default>(
    req: &AlkaneIdToOutpointRequest,
) -> Result<AlkaneIdToOutpointResponse> {
    let outpoint = alkane_id_to_outpoint_util::<E>(&req.id)?;
    let hex_string = outpoint.txid.to_string();
    Ok(AlkaneIdToOutpointResponse {
        txid: hex::decode(hex_string).unwrap(),
        vout: outpoint.vout,
    })
}

pub fn get_inventory<E: RuntimeEnvironment + Clone + Default>(
    req: &AlkaneInventoryRequest,
) -> Result<AlkaneInventoryResponse> {
    let alkane_inventory = alkane_inventory_pointer::<E>(&req.id);
    let alkanes = alkane_inventory
        .get_list()
        .into_iter()
        .map(|alkane_held| {
            let id = AlkaneId::parse(&mut Cursor::new(alkane_held.as_ref().clone())).unwrap();
            let balance_pointer = balance_pointer(
                &mut AtomicPointer::<AlkaneMessageContext<E>>::default(),
                &req.id,
                &id,
            );
            let balance = balance_pointer.get_value::<u128>();
            AlkaneTransfer {
                id,
                value: balance,
            }
        })
        .collect::<Vec<AlkaneTransfer>>();
    Ok(AlkaneInventoryResponse { alkanes })
}

pub fn get_storage_at<E: RuntimeEnvironment + Clone + Default>(
    req: &AlkaneStorageRequest,
) -> Result<AlkaneStorageResponse> {
    let alkane_storage_pointer = IndexPointer::<AlkaneMessageContext<E>>::from_keyword("/alkanes/")
        .select(&req.id.into())
        .keyword("/storage/")
        .select(&req.path);
    let value = alkane_storage_pointer.get().to_vec();
    Ok(AlkaneStorageResponse { value })
}

pub fn get_bytecode<E: RuntimeEnvironment + Clone + Default>(
    req: &BytecodeRequest,
) -> Result<BytecodeResponse> {
    let bytecode_ptr = IndexPointer::<AlkaneMessageContext<E>>::from_keyword("/alkanes/")
        .select(&req.id.into());
    let bytecode = bytecode_ptr.get();
    if bytecode.len() > 0 {
        Ok(BytecodeResponse {
            bytecode: alkanes_support::gz::decompress(bytecode.to_vec())?,
        })
    } else {
        Err(anyhow!("No bytecode found for the given AlkaneId"))
    }
}

pub fn get_block(req: &BlockRequest) -> Result<BlockResponse> {
    use crate::etl;
    let block = etl::get_block(req.height as u32)?;;
    Ok(BlockResponse {
        block,
        height: req.height,
    })
}

pub fn trace(outpoint: &OutPoint) -> Result<super::trace_types::AlkanesTrace> {
    let trace_bytes = TRACES
        .select(&consensus_encode::<OutPoint>(&outpoint)?)
        .get();
    let trace: super::trace_types::AlkanesTrace = bincode::deserialize(trace_bytes.as_ref())?;
    Ok(trace)
}

pub fn trace_block<E: RuntimeEnvironment + Clone + Default>(
    height: u32,
) -> Result<super::trace_types::AlkanesBlockTraceEvent> {
    let mut block_events: Vec<super::trace_types::AlkanesBlockEvent> = vec![];
    for outpoint in TRACES_BY_HEIGHT.select_value(height as u64).get_list() {
        let op = outpoint.clone().to_vec();
        let outpoint_decoded = OutPoint::consensus_decode(&mut &*op)?;
        let txid = outpoint_decoded.txid.to_string().as_bytes().to_vec();
        let txindex: u32 = RuneTable::<AlkaneMessageContext<E>>::new()
            .TXID_TO_TXINDEX
            .select(&txid)
            .get_value();
        let trace_bytes = TRACES.select(outpoint.as_ref()).get();
        let trace: super::trace_types::AlkanesTrace = bincode::deserialize(trace_bytes.as_ref())?;
        let block_event = super::trace_types::AlkanesBlockEvent {
            txindex: txindex as u64,
            outpoint: outpoint_decoded,
            traces: trace,
        };
        block_events.push(block_event);
    }

    Ok(super::trace_types::AlkanesBlockTraceEvent { events: block_events })
}