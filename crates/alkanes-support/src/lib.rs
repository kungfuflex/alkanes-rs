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
            special_fields: ::protobuf::SpecialFields::new(),
        }
    }
}

impl Into<proto::alkanes::AlkaneId> for AlkaneId {
    fn into(self) -> proto::alkanes::AlkaneId {
        proto::alkanes::AlkaneId {
            block: protobuf::MessageField::some(self.block.into()),
            tx: protobuf::MessageField::some(self.tx.into()),
            special_fields: ::protobuf::SpecialFields::new(),
        }
    }
}

impl Into<AlkaneId> for proto::alkanes::AlkaneId {
    fn into(self) -> AlkaneId {
        AlkaneId {
            block: self.block.unwrap_or_default().into(),
            tx: self.tx.unwrap_or_default().into(),
        }
    }
}

impl Into<proto::alkanes::AlkaneTransfer> for AlkaneTransfer {
    fn into(self) -> proto::alkanes::AlkaneTransfer {
        proto::alkanes::AlkaneTransfer {
            id: protobuf::MessageField::some(self.id.into()),
            value: protobuf::MessageField::some(self.value.into()),
            special_fields: ::protobuf::SpecialFields::new(),
        }
    }
}

impl Into<AlkaneTransfer> for proto::alkanes::AlkaneTransfer {
    fn into(self) -> AlkaneTransfer {
        AlkaneTransfer {
            id: self.id.into_option().map_or(AlkaneId::default(), |v| v.into()),
            value: self.value.into_option().map_or(0, |v| v.into()),
        }
    }
}

impl Into<proto::alkanes::ExtendedCallResponse> for ExtendedCallResponse {
    fn into(self) -> proto::alkanes::ExtendedCallResponse {
        proto::alkanes::ExtendedCallResponse {
            storage: self
                .storage
                .0
                .into_iter()
                .map(|(key, value)| proto::alkanes::KeyValuePair { key, value, special_fields: ::protobuf::SpecialFields::new() })
                .collect(),
            data: self.data,
            alkanes: self
                .alkanes
                .0
                .into_iter()
                .map(|v| v.into())
                .collect(),
            special_fields: ::protobuf::SpecialFields::new(),
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
                    .map(|transfer| AlkaneTransfer {
                        id: transfer.id.into_option().map_or(AlkaneId::default(), |v| v.into()),
                        value: transfer.value.into_option().map_or(0, |v| v.into()),
                    })
                    .collect::<Vec<AlkaneTransfer>>(),
            ),
        }
    }
}

impl Into<ProtoruneRuneId> for proto::alkanes::AlkaneId {
    fn into(self) -> ProtoruneRuneId {
        ProtoruneRuneId {
            block: self.block.unwrap_or_default().into(),
            tx: self.tx.unwrap_or_default().into(),
        }
    }
}

impl Into<proto::alkanes::AlkaneInventoryRequest> for AlkaneId {
    fn into(self) -> proto::alkanes::AlkaneInventoryRequest {
        proto::alkanes::AlkaneInventoryRequest { id: protobuf::MessageField::some(self.into()), special_fields: ::protobuf::SpecialFields::new() }
    }
}
