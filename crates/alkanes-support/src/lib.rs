pub mod cellpack;
pub mod constants;
pub mod context;
pub mod envelope;
pub mod gz;
pub mod id;
pub mod parcel;
pub mod proto;
pub mod response;
pub mod storage;
pub mod trace;
pub mod utils;
pub mod witness;

use crate::id::AlkaneId;
use crate::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use crate::response::ExtendedCallResponse;
use crate::storage::StorageMap;
use protorune_support::balance_sheet::ProtoruneRuneId;

impl From<proto::alkanes::Uint128> for u128 {
    fn from(v: proto::alkanes::Uint128) -> u128 {
        let mut result: Vec<u8> = Vec::<u8>::with_capacity(16);
        result.extend(&v.lo.to_le_bytes());
        result.extend(&v.hi.to_le_bytes());
        let bytes_ref: &[u8] = &result;
        u128::from_le_bytes(bytes_ref.try_into().unwrap())
    }
}

impl From<u128> for proto::alkanes::Uint128 {
    fn from(v: u128) -> proto::alkanes::Uint128 {
        let bytes = v.to_le_bytes().to_vec();
        proto::alkanes::Uint128 {
            lo: u64::from_le_bytes((&bytes[0..8]).try_into().unwrap()),
            hi: u64::from_le_bytes((&bytes[8..16]).try_into().unwrap()),
        }
    }
}

impl From<AlkaneId> for proto::alkanes::AlkaneId {
    fn from(val: AlkaneId) -> Self {
        proto::alkanes::AlkaneId {
            block: Some(val.block.into()),
            tx: Some(val.tx.into()),
        }
    }
}

impl From<proto::alkanes::AlkaneId> for AlkaneId {
    fn from(val: proto::alkanes::AlkaneId) -> Self {
        AlkaneId {
            block: val.block.map_or(0, |v| v.into()),
            tx: val.tx.map_or(0, |v| v.into()),
        }
    }
}

impl From<AlkaneTransfer> for proto::alkanes::AlkaneTransfer {
    fn from(val: AlkaneTransfer) -> Self {
        proto::alkanes::AlkaneTransfer {
            id: Some(val.id.into()),
            value: Some(val.value.into()),
        }
    }
}

impl From<proto::alkanes::AlkaneTransfer> for AlkaneTransfer {
    fn from(val: proto::alkanes::AlkaneTransfer) -> Self {
        AlkaneTransfer {
            id: val.id.map_or(AlkaneId::default(), |v| v.into()),
            value: val.value.map_or(0, |v| v.into()),
        }
    }
}

impl From<ExtendedCallResponse> for proto::alkanes::ExtendedCallResponse {
    fn from(val: ExtendedCallResponse) -> Self {
        proto::alkanes::ExtendedCallResponse {
            storage: val
                .storage
                .0
                .into_iter()
                .map(|(key, value)| proto::alkanes::KeyValuePair { key, value })
                .collect::<Vec<proto::alkanes::KeyValuePair>>(),
            data: val.data,
            alkanes: val
                .alkanes
                .0
                .into_iter()
                .map(|v| v.into())
                .collect::<Vec<proto::alkanes::AlkaneTransfer>>(),
        }
    }
}

impl From<proto::alkanes::ExtendedCallResponse> for ExtendedCallResponse {
    fn from(v: proto::alkanes::ExtendedCallResponse) -> ExtendedCallResponse {
        ExtendedCallResponse {
            storage: StorageMap::from_iter(v.storage.into_iter().map(|kv| (kv.key, kv.value))),
            data: v.data,
            alkanes: AlkaneTransferParcel(
                v.alkanes
                    .into_iter()
                    .map(|transfer| transfer.into())
                    .collect::<Vec<AlkaneTransfer>>(),
            ),
        }
    }
}

impl From<proto::alkanes::AlkaneId> for ProtoruneRuneId {
    fn from(val: proto::alkanes::AlkaneId) -> Self {
        ProtoruneRuneId {
            block: val.block.map_or(0, |v| v.into()),
            tx: val.tx.map_or(0, |v| v.into()),
        }
    }
}

impl From<AlkaneId> for proto::alkanes::AlkaneInventoryRequest {
    fn from(val: AlkaneId) -> Self {
        proto::alkanes::AlkaneInventoryRequest {
            id: Some(val.into()),
        }
    }
}
