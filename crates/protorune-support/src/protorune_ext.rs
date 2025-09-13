use crate::proto::protorune::{ProtoruneRuneId, Uint128};
use protobuf::Message;
use std::cmp::Ordering;

impl Eq for ProtoruneRuneId {}

impl Ord for ProtoruneRuneId {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_bytes = self.write_to_bytes().unwrap();
        let other_bytes = other.write_to_bytes().unwrap();
        self_bytes.cmp(&other_bytes)
    }
}

impl PartialOrd for ProtoruneRuneId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<ordinals::RuneId> for ProtoruneRuneId {
    fn from(rune_id: ordinals::RuneId) -> Self {
        Self {
            height: Some((rune_id.block as u128).into()).into(),
            txindex: Some((rune_id.tx as u128).into()).into(),
            ..Default::default()
        }
    }
}

impl From<ProtoruneRuneId> for Vec<u8> {
    fn from(val: ProtoruneRuneId) -> Self {
        val.write_to_bytes().unwrap()
    }
}

impl TryFrom<Vec<u8>> for ProtoruneRuneId {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(ProtoruneRuneId::parse_from_bytes(&value)?)
    }
}

impl From<u128> for Uint128 {
    fn from(value: u128) -> Self {
        Self {
            lo: value as u64,
            hi: (value >> 64) as u64,
            ..Default::default()
        }
    }
}

impl From<Uint128> for u128 {
    fn from(value: Uint128) -> Self {
        (value.hi as u128) << 64 | value.lo as u128
    }
}