use crate::indexer::configure_network;
use crate::view::{parcel_from_protobuf, simulate_safe};
use alkanes_support::proto;
use bitcoin::{Block, OutPoint};
#[allow(unused_imports)]
use metashrew::{
    flush, input, println,
    stdio::{stdout, Write},
};
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::export_bytes;
use metashrew_support::utils::{consensus_decode, consume_sized_int, consume_to_end};
use protobuf::{Message, MessageField};
use std::io::Cursor;
pub mod block;
pub mod indexer;
pub mod message;
pub mod network;
pub mod precompiled;
pub mod tables;
#[cfg(any(test, feature = "test-utils"))]
pub mod tests;
pub mod trace;
pub mod utils;
pub mod view;
pub mod vm;
use crate::indexer::index_block;

#[no_mangle]
pub fn simulate() -> i32 {
    configure_network();
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::new();
    match simulate_safe(
        &parcel_from_protobuf(
            proto::alkanes::MessageContextParcel::parse_from_bytes(reader).unwrap(),
        ),
        u64::MAX,
    ) {
        Ok((response, gas_used)) => {
            result.execution = MessageField::some(response.into());
            result.gas_used = gas_used;
        }
        Err(e) => {
            result.error = e.to_string();
        }
    }
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn runesbyaddress() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::WalletResponse =
        protorune::view::runes_by_address(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn runesbyoutpoint() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse =
        protorune::view::runes_by_outpoint(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn spendablesbyaddress() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::WalletResponse =
        view::protorunes_by_address(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn protorunesbyaddress() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let mut result: protorune_support::proto::protorune::WalletResponse =
        view::protorunes_by_address(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    result.outpoints = result
        .outpoints
        .into_iter()
        .filter_map(|v| {
            if v.clone()
                .balances
                .unwrap_or_else(|| protorune_support::proto::protorune::BalanceSheet::new())
                .entries
                .len()
                == 0
            {
                None
            } else {
                Some(v)
            }
        })
        .collect::<Vec<protorune_support::proto::protorune::OutpointResponse>>();
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn protorunesbyheight() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse =
        view::protorunes_by_height(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::RunesResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn trace() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let outpoint: OutPoint = protorune_support::proto::protorune::Outpoint::parse_from_bytes(
        &consume_to_end(&mut data).unwrap(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    export_bytes(view::trace(&outpoint).unwrap())
}

#[no_mangle]
pub fn protorunesbyoutpoint() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse =
        view::protorunes_by_outpoint(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::new());

    export_bytes(result.write_to_bytes().unwrap())
}

#[no_mangle]
pub fn runesbyheight() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse =
        protorune::view::runes_by_height(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| protorune_support::proto::protorune::RunesResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
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
//

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub fn _start() {
    let data = input();
    let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
    let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec()))
        .unwrap()
        .to_consensus();
    #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
    let block: Block =
        consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();

    index_block(&block, height).unwrap();
    flush();
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::message::AlkaneMessageContext;
    use protobuf::{Message, SpecialFields};
    use protorune::view::{rune_outpoint_to_outpoint_response, runes_by_address, runes_by_height};
    use protorune::Protorune;
    use protorune_support::proto::protorune::{RunesByHeightRequest, Uint128, WalletRequest};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    pub fn test_decode_block() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src/tests/static/849236.txt");
        let block_data = fs::read(&path).unwrap();

        assert!(block_data.len() > 0);

        let data = block_data;
        let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
        let reader = &data[4..];
        let block: Block =
            consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();
        assert!(height == 849236);

        // calling index_block directly fails since genesis(&block).unwrap(); gets segfault
        // index_block(&block, height).unwrap();
        configure_network();
        Protorune::index_block::<AlkaneMessageContext>(block.clone(), height.into()).unwrap();

        let req_height: Vec<u8> = (RunesByHeightRequest {
            height: 849236,
            special_fields: SpecialFields::new(),
        })
        .write_to_bytes()
        .unwrap();
        let runes = runes_by_height(&req_height).unwrap();
        assert!(runes.runes.len() == 2);

        // TODO: figure out what address to use for runesbyaddress
        let req_wallet: Vec<u8> = (WalletRequest {
            wallet: String::from("bc1pfs5dhzwk32xa53cjx8fx4dqy7hm4m6tys8zyvemqffz8ua4tytqs8vjdgr")
                .as_bytes()
                .to_vec(),
            special_fields: SpecialFields::new(),
        })
        .write_to_bytes()
        .unwrap();

        let runes_for_addr = runes_by_address(&req_wallet).unwrap();
        // assert!(runes_for_addr.balances > 0);
        std::println!("RUNES by addr: {:?}", runes_for_addr);

        let outpoint_res = rune_outpoint_to_outpoint_response(&OutPoint {
            txid: block.txdata[298].compute_txid(),
            vout: 2,
        })
        .unwrap();
        let quorum_rune = outpoint_res.balances.unwrap().entries[0].clone();
        let balance = quorum_rune.balance.0.unwrap();
        let mut expected_balance = Uint128::new();
        expected_balance.lo = 21000000;
        assert!(*balance == expected_balance);
        // TODO: Assert rune
        std::println!(" with rune {:?}", quorum_rune.rune.0);

        // assert!(false);
    }
}
