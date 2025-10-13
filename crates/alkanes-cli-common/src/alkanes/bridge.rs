use crate::alkanes_pb;
use protorune_support::proto::protorune as protorune_pb;

pub fn from_prost_uint128(v: protorune_pb::Uint128) -> u128 {
    let mut result: Vec<u8> = Vec::<u8>::with_capacity(16);
    result.extend(&v.lo.to_le_bytes());
    result.extend(&v.hi.to_le_bytes());
    let bytes_ref: &[u8] = &result;
    u128::from_le_bytes(bytes_ref.try_into().unwrap())
}

pub fn to_prost_uint128(v: u128) -> protorune_pb::Uint128 {
    let bytes = v.to_le_bytes().to_vec();
    let mut container = protorune_pb::Uint128::default();
    container.lo = u64::from_le_bytes((&bytes[0..8]).try_into().unwrap());
    container.hi = u64::from_le_bytes((&bytes[8..16]).try_into().unwrap());
    container
}
