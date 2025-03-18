use crate::indexer::configure_network;
use crate::view::{multi_simulate_safe, parcel_from_protobuf, simulate_safe};
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
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{consensus_decode, consume_sized_int, consume_to_end};
use protobuf::{Message, MessageField};
use std::io::Cursor;
use view::parcels_from_protobuf;
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

/*
All the #[no_mangle] configs will fail during github action cargo test step
due to duplicate symbol:
  rust-lld: error: duplicate symbol: runesbyheight
  >>> defined in /home/runner/work/alkanes-rs/alkanes-rs/target/wasm32-unknown-unknown/debug/deps/alkanes-5b647d16704125c9.alkanes.7a19fa39330b2460-cgu.05.rcgu.o
  >>> defined in /home/runner/work/alkanes-rs/alkanes-rs/target/wasm32-unknown-unknown/debug/deps/libalkanes.rlib(alkanes.alkanes.2dae95da706e3a8c-cgu.09.rcgu.o)

This is because both
[lib]
crate-type = ["cdylib", "rlib"]

are defined in Cargo.toml since we want to build both the wasm and rust library.

Running cargo test will compile an additional test harness binary that:
Links libalkanes.rlib
Compiles #[no_mangle] functions again into the test binary
Then links everything together, leading to duplicate symbols

Thus, going to add not(test) to all these functions
*/

#[cfg(not(test))]
#[no_mangle]
pub fn multisimluate() -> i32 {
    configure_network();
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::MultiSimulateResponse =
        proto::alkanes::MultiSimulateResponse::new();
    let responses = multi_simulate_safe(
        &parcels_from_protobuf(
            proto::alkanes::MultiSimulateRequest::parse_from_bytes(reader).unwrap(),
        ),
        u64::MAX,
    );

    for response in responses {
        let mut res = proto::alkanes::SimulateResponse::new();
        match response {
            Ok((response, gas_used)) => {
                res.execution = MessageField::some(response.into());
                res.gas_used = gas_used;
            }
            Err(e) => {
                result.error = e.to_string();
            }
        }
        result.responses.push(res);
    }

    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
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

#[cfg(not(test))]
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

#[cfg(not(test))]
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

// #[cfg(not(test))]
// #[no_mangle]
// pub fn spendablesbyaddress() -> i32 {
//     configure_network();
//     let mut data: Cursor<Vec<u8>> = Cursor::new(input());
//     let _height = consume_sized_int::<u32>(&mut data).unwrap();
//     let result: protorune_support::proto::protorune::WalletResponse =
//         view::protorunes_by_address(&consume_to_end(&mut data).unwrap())
//             .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
//     export_bytes(result.write_to_bytes().unwrap())
// }

// #[cfg(not(test))]
// #[no_mangle]
// pub fn spendablesbyaddress2() -> i32 {
//     configure_network();
//     let mut data: Cursor<Vec<u8>> = Cursor::new(input());
//     let _height = consume_sized_int::<u32>(&mut data).unwrap();
//     let result: protorune_support::proto::protorune::WalletResponse =
//         view::protorunes_by_address2(&consume_to_end(&mut data).unwrap())
//             .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
//     export_bytes(result.write_to_bytes().unwrap())
// }

// #[cfg(not(test))]
// #[no_mangle]
// pub fn protorunesbyaddress() -> i32 {
//     configure_network();
//     let mut data: Cursor<Vec<u8>> = Cursor::new(input());
//     let _height = consume_sized_int::<u32>(&mut data).unwrap();

//     let input_data = consume_to_end(&mut data).unwrap();
//     let request = protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(&input_data).unwrap();

//     #[cfg(feature = "cache")]
//     {
//         // Check if we have a cached filtered response for this address
//         let cached_response = protorune::tables::CACHED_FILTERED_WALLET_RESPONSE.select(&request.wallet).get();

//         if !cached_response.is_empty() {
//             // Use the cached filtered response if available
//             match protorune_support::proto::protorune::WalletResponse::parse_from_bytes(&cached_response) {
//                 Ok(response) => {
//                     return export_bytes(response.write_to_bytes().unwrap());
//                 },
//                 Err(e) => {
//                     println!("Error parsing cached filtered wallet response: {:?}", e);
//                     // Fall back to computing the response if parsing fails
//                 }
//             }
//         }
//     }

//     // If no cached response or parsing failed, compute it
//     let mut result: protorune_support::proto::protorune::WalletResponse =
//         view::protorunes_by_address(&input_data)
//             .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());

//     // Filter the outpoints to only include those with runes
//     result.outpoints = result
//         .outpoints
//         .into_iter()
//         .filter_map(|v| {
//             if v.clone()
//                 .balances
//                 .unwrap_or_else(|| protorune_support::proto::protorune::BalanceSheet::new())
//                 .entries
//                 .len()
//                 == 0
//             {
//                 None
//             } else {
//                 Some(v)
//             }
//         })
//         .collect::<Vec<protorune_support::proto::protorune::OutpointResponse>>();

//     export_bytes(result.write_to_bytes().unwrap())
// }

// #[cfg(not(test))]
// #[no_mangle]
// pub fn protorunesbyaddress2() -> i32 {
//     configure_network();
//     let mut data: Cursor<Vec<u8>> = Cursor::new(input());
//     let _height = consume_sized_int::<u32>(&mut data).unwrap();

//     let input_data = consume_to_end(&mut data).unwrap();
//     let request = protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(&input_data).unwrap();

//     #[cfg(feature = "cache")]
//     {
//         // Check if we have a cached filtered response for this address
//         let cached_response = protorune::tables::CACHED_FILTERED_WALLET_RESPONSE.select(&request.wallet).get();

//         if !cached_response.is_empty() {
//             // Use the cached filtered response if available
//             match protorune_support::proto::protorune::WalletResponse::parse_from_bytes(&cached_response) {
//                 Ok(response) => {
//                     return export_bytes(response.write_to_bytes().unwrap());
//                 },
//                 Err(e) => {
//                     println!("Error parsing cached filtered wallet response: {:?}", e);
//                     // Fall back to computing the response if parsing fails
//                 }
//             }
//         }
//     }

//     // If no cached response or parsing failed, compute it
//     let mut result: protorune_support::proto::protorune::WalletResponse =
//         view::protorunes_by_address2(&input_data)
//             .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());

//     // Filter the outpoints to only include those with runes
//     result.outpoints = result
//         .outpoints
//         .into_iter()
//         .filter_map(|v| {
//             if v.clone()
//                 .balances
//                 .unwrap_or_else(|| protorune_support::proto::protorune::BalanceSheet::new())
//                 .entries
//                 .len()
//                 == 0
//             {
//                 None
//             } else {
//                 Some(v)
//             }
//         })
//         .collect::<Vec<protorune_support::proto::protorune::OutpointResponse>>();

//     export_bytes(result.write_to_bytes().unwrap())
// }

#[cfg(not(test))]
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

#[cfg(not(test))]
#[no_mangle]
pub fn traceblock() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(view::traceblock(height).unwrap())
}

#[cfg(not(test))]
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

#[cfg(not(test))]
#[no_mangle]
pub fn getbytecode() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(view::getbytecode(&consume_to_end(&mut data).unwrap()).unwrap_or_default())
}

#[cfg(not(test))]
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

#[cfg(not(test))]
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

#[cfg(all(target_arch = "wasm32", not(test)))]
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
