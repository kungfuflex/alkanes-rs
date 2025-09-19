use crate::indexer::configure_network;


use bitcoin::{Block, OutPoint};
use metashrew_core::environment::MetashrewEnvironment;
use metashrew_support::environment::RuntimeEnvironment;
use alkanes_support::proto;
use metashrew_support::compat::{export_bytes};
use metashrew_support::utils::{consume_to_end, consume_sized_int, consensus_decode};
use crate::view::{multi_simulate_safe, simulate_safe, meta_safe, parcels_from_protobuf, parcel_from_protobuf};
use prost::Message;

use std::io::Cursor;
pub mod block;
pub mod etl;
pub mod indexer;
pub mod message;
pub mod network;
pub mod precompiled;
pub mod tables;
#[cfg(any(test, feature = "test-utils"))]
pub mod tests;
pub mod trace;
pub mod unwrap;
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
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let data = env.load_input().unwrap().data;
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::MultiSimulateResponse =
        proto::alkanes::MultiSimulateResponse::default();
    let responses = multi_simulate_safe(
        &parcels_from_protobuf::<MetashrewEnvironment>(
            proto::alkanes::MultiSimulateRequest::decode(reader).unwrap(),
        ),
        u64::MAX,
    );

    for response in responses {
        let mut res = proto::alkanes::SimulateResponse::default();
        match response {
            Ok((response, gas_used)) => {
                res.execution = Some(response.into());
                res.gas_used = gas_used;
            }
            Err(e) => {
                result.error = e.to_string();
            }
        }
        result.responses.push(res);
    }

    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn simulate() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let data = env.load_input().unwrap().data;
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::default();
    match simulate_safe(
        &parcel_from_protobuf::<MetashrewEnvironment>(
            proto::alkanes::MessageContextParcel::decode(reader).unwrap(),
        ),
        u64::MAX,
    ) {
        Ok((response, gas_used)) => {
            result.execution = Some(response.into());
            result.gas_used = gas_used;
        }
        Err(e) => {
            result.error = e.to_string();
        }
    }
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn sequence() -> i32 {
    let mut env = MetashrewEnvironment::default();
    export_bytes(view::sequence::<MetashrewEnvironment>().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn meta() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let data = env.load_input().unwrap().data;
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    match meta_safe(
        &parcel_from_protobuf::<MetashrewEnvironment>(
            proto::alkanes::MessageContextParcel::decode(reader).unwrap(),
        ),
    ) {
        Ok(response) => export_bytes(response),
        Err(_) => export_bytes(vec![]),
    }
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyaddress() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::WalletResponse =
        protorune::view::runes_by_address::<MetashrewEnvironment>(
            &consume_to_end(&mut data).unwrap(),
            &mut env,
        )
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::default());
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn unwrap() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(view::unwrap::<MetashrewEnvironment>(height.into()).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyoutpoint() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse =
        protorune::view::runes_by_outpoint::<MetashrewEnvironment>(
            &consume_to_end(&mut data).unwrap(),
            &mut env,
        )
        .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::default());
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn spendablesbyaddress() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::WalletResponse =
        view::protorunes_by_address::<MetashrewEnvironment>(
            &consume_to_end(&mut data).unwrap(),
        )
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::default());
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyaddress() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let input_data = consume_to_end(&mut data).unwrap();
    //  let _request = protorune_support::proto::protorune::ProtorunesWalletRequest::decode(&input_data).unwrap();

    let mut result: protorune_support::proto::protorune::WalletResponse = view::protorunes_by_address::<MetashrewEnvironment>(&input_data)
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::default());

    result.outpoints = result
        .outpoints
        .into_iter()
        .filter_map(|v| {
            if v.clone()
                .balances
                .unwrap_or_default()
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

    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getblock() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let input_data = consume_to_end(&mut data).unwrap();
    export_bytes(view::getblock(&input_data).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyheight() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse = view::protorunes_by_height::<MetashrewEnvironment>(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::RunesResponse::default());
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn alkanes_id_to_outpoint() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    // first 4 bytes come in as height, not used
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let data_vec = consume_to_end(&mut data).unwrap();
    let result: alkanes_support::proto::alkanes::AlkaneIdToOutpointResponse = view::alkanes_id_to_outpoint::<MetashrewEnvironment>(&data_vec).unwrap_or_else(|err| {
        eprintln!("Error in alkanes_id_to_outpoint: {:?}", err);
        alkanes_support::proto::alkanes::AlkaneIdToOutpointResponse::default()
    });
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn traceblock() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(view::traceblock::<MetashrewEnvironment>(height).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn trace() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let outpoint: OutPoint = protorune_support::proto::protorune::Outpoint::decode(
        consume_to_end(&mut data).unwrap().as_slice(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    export_bytes(view::trace(&outpoint).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getbytecode() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(
        view::getbytecode::<MetashrewEnvironment>(&consume_to_end(&mut data).unwrap()).unwrap_or_default(),
    )
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyoutpoint() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse = view::protorunes_by_outpoint::<MetashrewEnvironment>(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::default());

    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyheight() -> i32 {
    let mut env = MetashrewEnvironment::default();
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(env.load_input().unwrap().data);
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse = protorune::view::runes_by_height::<MetashrewEnvironment>(&consume_to_end(&mut data).unwrap(), &mut env)
        .unwrap_or_else(|_| protorune_support::proto::protorune::RunesResponse::default());
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getinventory() -> i32 {
    let mut env = MetashrewEnvironment::default();
    let data = env.load_input().unwrap().data;
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let result = view::getinventory::<MetashrewEnvironment>(
        &proto::alkanes::AlkaneInventoryRequest::decode(reader)
            .unwrap()
            .into(),
    )
    .unwrap();
    export_bytes(result.encode_to_vec())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getstorageat() -> i32 {
    let mut env = MetashrewEnvironment::default();
    let data = env.load_input().unwrap().data;
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let result = view::getstorageat::<MetashrewEnvironment>(
        &proto::alkanes::AlkaneStorageRequest::decode(reader)
            .unwrap()
            .into(),
    )
    .unwrap();
    export_bytes(result.encode_to_vec())
}

#[cfg(all(target_arch = "wasm32", not(test)))]
#[no_mangle]
pub fn _start() {
    let mut env = MetashrewEnvironment::default();
    let data = env.load_input().unwrap().data;
    let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
    let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec()))
        .unwrap()
        .to_consensus();
    #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
    let block: Block =
        consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();

    index_block::<MetashrewEnvironment>(&mut env, &block, height).unwrap();
    etl::index_extensions(&mut env, height, &block);
    env.flush(&[]).unwrap();
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::message::AlkaneMessageContext;
    use prost::Message;
    use protorune::view::{protorune_outpoint_to_outpoint_response, runes_by_address, runes_by_height};
    use protorune::Protorune;
    use protorune_support::proto::protorune::{RunesByHeightRequest, Uint128, WalletRequest};
    
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    pub fn test_decode_block() {
        let mut env = MetashrewEnvironment::default();
        let block_data = include_bytes!("tests/static/849236.txt").to_vec();

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
        Protorune::index_block::<AlkaneMessageContext<MetashrewEnvironment>>(
            &mut env,
            block.clone(),
            height.into(),
        )
        .unwrap();

        let req_height: Vec<u8> = (RunesByHeightRequest {
            height: 849236,
        })
        .encode_to_vec();
        let runes = runes_by_height::<MetashrewEnvironment>(&mut env, &req_height).unwrap();
        assert!(runes.runes.len() == 2);

        // TODO: figure out what address to use for runesbyaddress
        let req_wallet: Vec<u8> = (WalletRequest {
            wallet: String::from("bc1pfs5dhzwk32xa53cjx8fx4dqy7hm4m6tys8zyvemqffz8ua4tytqs8vjdgr")
                .as_bytes()
                .to_vec(),
        })
        .encode_to_vec();

        let runes_for_addr = runes_by_address::<MetashrewEnvironment>(&mut env, &req_wallet).unwrap();
        // assert!(runes_for_addr.balances > 0);
        std::println!("RUNES by addr: {:?}", runes_for_addr);

        let outpoint_res = protorune_outpoint_to_outpoint_response::<MetashrewEnvironment>(
            &mut env,
            &(OutPoint {
                txid: block.txdata[298].compute_txid(),
                vout: 2,
            }),
            0,
        )
        .unwrap();
        let quorum_rune = outpoint_res.balances.unwrap().entries[0].clone();
        let balance = quorum_rune.balance.unwrap();
        let mut expected_balance = Uint128::default();
        expected_balance.lo = 21000000;
        assert!(balance == expected_balance);
        // TODO: Assert rune
        std::println!(" with rune {:?}", quorum_rune.rune.unwrap());

        // assert!(false);
    }
}