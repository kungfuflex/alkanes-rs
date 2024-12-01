use crate::view::simulate_parcel;
use alkanes_support::proto;
use bitcoin::hashes::Hash;
use bitcoin::{Transaction, blockdata::block::{Header}, Block, BlockHash, CompactTarget, TxMerkleNode};
use bitcoin::blockdata::transaction::{Version};
#[allow(unused_imports)]
use metashrew::{
    flush, input, println,
    stdio::{stdout, Write},
};
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use protobuf::{Message, MessageField};
use protorune::message::MessageContextParcel;
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_decode;
use std::io::Cursor;
pub mod message;
pub mod indexer;
pub mod network;
pub mod precompiled;
#[cfg(test)]
pub mod tests;
pub mod utils;
pub mod view;
pub mod vm;
use crate::indexer::{index_block};

fn default_transaction() -> Transaction {
    Transaction {
        version: Version::non_standard(0),
        lock_time: bitcoin::absolute::LockTime::from_consensus(0),
        input: vec![],
        output: vec![],
    }
}

fn default_block() -> Block {
    Block {
        header: Header {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![],
    }
}

pub fn parcel_from_protobuf(v: proto::alkanes::MessageContextParcel) -> MessageContextParcel {
    let mut result = MessageContextParcel::default();
    result.height = v.height;
    result.block = if v.block.len() > 0 {
        consensus_decode::<Block>(&mut Cursor::new(v.block)).unwrap()
    } else {
        default_block()
    };
    result.transaction = if v.transaction.len() > 0 {
        consensus_decode::<Transaction>(&mut Cursor::new(v.transaction)).unwrap()
    } else {
        default_transaction()
    };
    result.vout = v.vout;
    result.calldata = v.calldata;
    result.runes = v
        .alkanes
        .into_iter()
        .map(|v| RuneTransfer {
            id: v.id.into_option().unwrap().clone().into(),
            value: v.value.into_option().unwrap().into(),
        })
        .collect::<Vec<RuneTransfer>>();
    result.pointer = v.pointer;
    result.refund_pointer = v.refund_pointer;
    result
}

#[no_mangle]
pub fn simulate() -> i32 {
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::new();
    match simulate_parcel(&parcel_from_protobuf(
        proto::alkanes::MessageContextParcel::parse_from_bytes(reader).unwrap(),
    )) {
        Ok((response, gas_used)) => {
            result.execution = MessageField::some(response.into());
            result.gas_used = gas_used;
        }
        Err(e) => {
            result.error = e.to_string();
        }
    }
    to_passback_ptr(&mut to_arraybuffer_layout::<&[u8]>(
        result.write_to_bytes().unwrap().as_ref(),
    ))
}

// #[no_mangle]
// pub fn alkane_balance_sheet() -> i32 {
//     let data = input();
//     let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
//     let reader = &data[4..];
//     let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::new();
//     let (response, gas_used) = alkane_inventory(
//         &proto::alkanes::MessageContextParcel::parse_from_bytes(reader).unwrap().into()
//     ).unwrap();
//     result.execution = MessageField::some(response.into());
//     result.gas_used = gas_used;
//     to_passback_ptr(&mut to_arraybuffer_layout::<&[u8]>(result.write_to_bytes().unwrap().as_ref()))
// }
//

#[no_mangle]
pub fn _start() {
    let data = input();
    let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec()))
        .unwrap()
        .to_consensus();
    index_block(&block, height).unwrap();
    flush();
}
