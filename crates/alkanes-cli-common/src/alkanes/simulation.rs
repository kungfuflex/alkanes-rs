//! Simulation functionality for alkanes operations

#[cfg(not(target_arch = "wasm32"))]
use std::{vec::Vec};
#[cfg(target_arch = "wasm32")]
use alloc::{vec::Vec};

pub fn simulate_cellpack(cellpack: &[u32]) -> crate::alkanes_pb::MessageContextParcel {
    
    let _cellpack_as_u128: Vec<u128> = cellpack.iter().map(|&x| x as u128).collect();
crate::alkanes_pb::MessageContextParcel {
        alkanes: Vec::new(),
        calldata: Vec::new(),
        vout: 0,
        pointer: 0,
        txindex: 0,
        refund_pointer: 0,
        height: 880000,
        block: Vec::<u8>::default(),
transaction: Vec::new(),
        special_fields: Default::default(),
    }
}