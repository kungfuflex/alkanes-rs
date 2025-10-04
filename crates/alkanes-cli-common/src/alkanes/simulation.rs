//! Simulation functionality for alkanes operations

#[cfg(not(target_arch = "wasm32"))]
use std::{vec::Vec};
#[cfg(target_arch = "wasm32")]
use alloc::{vec::Vec};

pub fn simulate_cellpack(cellpack: &[u32]) -> alkanes_support::proto::alkanes::MessageContextParcel {
    use alkanes_support::cellpack::Cellpack;
    let cellpack_as_u128: Vec<u128> = cellpack.iter().map(|&x| x as u128).collect();
    alkanes_support::proto::alkanes::MessageContextParcel {
        vout: 0,
        pointer: 0,
        txindex: 0,
        refund_pointer: 0,
        height: 880000,
        block: Vec::<u8>::default(),
        transaction: Vec::<u8>::default(),
        alkanes: Vec::<alkanes_support::proto::alkanes::AlkaneTransfer>::default(),
        calldata: Cellpack::try_from(cellpack_as_u128).unwrap().encipher(),
        
    }
}