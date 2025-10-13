use alkanes_support::id::AlkaneId;
use alkanes_support::proto;
use bitcoin::{Block, OutPoint};
use bitcoin::hashes::Hash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::views::trace_types::{AlkanesBlockTraceEvent, AlkanesBlockEvent, AlkanesTrace, AlkanesTraceEvent, AlkanesTraceEvent_oneof_event, AlkanesEnterContext, AlkanesExitContext, AlkanesCreate, Context, KeyValuePair};
use serde_with::{serde_as, DisplayFromStr};

pub struct AlkaneIdToOutpointRequest {
    pub id: AlkaneId,
}

pub struct AlkaneIdToOutpointResponse {
    pub txid: Vec<u8>,
    pub vout: u32,
}

impl From<proto::alkanes::AlkaneIdToOutpointRequest> for AlkaneIdToOutpointRequest {
    fn from(request: proto::alkanes::AlkaneIdToOutpointRequest) -> Self {
        Self { id: request.id.unwrap().into() }
    }
}

impl From<AlkaneIdToOutpointResponse> for proto::alkanes::AlkaneIdToOutpointResponse {
    fn from(response: AlkaneIdToOutpointResponse) -> Self {
        let mut proto_response = proto::alkanes::AlkaneIdToOutpointResponse::default();
        proto_response.txid = response.txid;
        proto_response.vout = response.vout;
        proto_response
    }
}

pub struct AlkaneInventoryRequest {
    pub id: AlkaneId,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkaneTransfer {
    #[serde_as(as = "DisplayFromStr")]
    pub id: AlkaneId,
    pub value: u128,
}

pub struct AlkaneInventoryResponse {
    pub alkanes: Vec<AlkaneTransfer>,
}

impl From<proto::alkanes::AlkaneInventoryRequest> for AlkaneInventoryRequest {
    fn from(request: proto::alkanes::AlkaneInventoryRequest) -> Self {
        Self { id: request.id.unwrap().into() }
    }
}

impl From<AlkaneInventoryResponse> for proto::alkanes::AlkaneInventoryResponse {
    fn from(response: AlkaneInventoryResponse) -> Self {
        let mut proto_response = proto::alkanes::AlkaneInventoryResponse::default();
        proto_response.alkanes = response.alkanes.into_iter().map(|v| v.into()).collect();
        proto_response
    }
}

pub struct AlkaneStorageRequest {
    pub id: AlkaneId,
    pub path: Vec<u8>,
}

pub struct AlkaneStorageResponse {
    pub value: Vec<u8>,
}

impl From<proto::alkanes::AlkaneStorageRequest> for AlkaneStorageRequest {
    fn from(request: proto::alkanes::AlkaneStorageRequest) -> Self {
        Self {
            id: request.id.unwrap().into(),
            path: request.path,
        }
    }
}

impl From<AlkaneStorageResponse> for proto::alkanes::AlkaneStorageResponse {
    fn from(response: AlkaneStorageResponse) -> Self {
        let mut proto_response = proto::alkanes::AlkaneStorageResponse::default();
        proto_response.value = response.value;
        proto_response
    }
}

pub struct BytecodeRequest {
    pub id: AlkaneId,
}

pub struct BytecodeResponse {
    pub bytecode: Vec<u8>,
}

impl From<proto::alkanes::BytecodeRequest> for BytecodeRequest {
    fn from(request: proto::alkanes::BytecodeRequest) -> Self {
        Self { id: request.id.unwrap().into() }
    }
}

pub struct BlockRequest {
    pub height: u64,
}

pub struct BlockResponse {
    pub block: Block,
    pub height: u64,
}

impl From<proto::alkanes::BlockRequest> for BlockRequest {
    fn from(request: proto::alkanes::BlockRequest) -> Self {
        Self { height: request.height as u64 }
    }
}

impl From<BlockResponse> for proto::alkanes::BlockResponse {
    fn from(response: BlockResponse) -> Self {
        let mut proto_response = proto::alkanes::BlockResponse::default();
        proto_response.block = bitcoin::consensus::encode::serialize(&response.block);
        proto_response.height = response.height as u32;
        proto_response
    }
}

impl From<AlkaneTransfer> for proto::alkanes::AlkaneTransfer {
    fn from(transfer: AlkaneTransfer) -> Self {
        let mut proto_transfer = proto::alkanes::AlkaneTransfer::default();
        proto_transfer.id = Some(transfer.id.into());
        let mut uint128 = proto::alkanes::Uint128::default();
        uint128.lo = (transfer.value & u64::MAX as u128) as u64;
        uint128.hi = (transfer.value >> 64) as u64;
        proto_transfer.value = Some(uint128);
        proto_transfer
    }
}