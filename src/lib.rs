use crate::indexer::configure_network;
use crate::view::{ parcel_from_protobuf, simulate_safe };
use alkanes_support::proto;
use bitcoin::{ Block, OutPoint };
#[allow(unused_imports)]
use metashrew::{ flush, input, println, stdio::{ stdout, Write } };
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::export_bytes;
use metashrew_support::utils::{ consensus_decode, consume_sized_int, consume_to_end };
use protobuf::{ Message, MessageField };
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
pub fn simulate() -> i32 {
    configure_network();
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::new();
    match
        simulate_safe(
            &parcel_from_protobuf(
                proto::alkanes::MessageContextParcel::parse_from_bytes(reader).unwrap()
            ),
            u64::MAX
        )
    {
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
    let result: protorune_support::proto::protorune::WalletResponse = protorune::view
        ::runes_by_address(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyoutpoint() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse = protorune::view
        ::runes_by_outpoint(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn spendablesbyaddress() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::WalletResponse = view
        ::protorunes_by_address(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyaddress() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let mut result: protorune_support::proto::protorune::WalletResponse = view
        ::protorunes_by_address(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::WalletResponse::new());
    result.outpoints = result.outpoints
        .into_iter()
        .filter_map(|v| {
            if
                v
                    .clone()
                    .balances.unwrap_or_else(||
                        protorune_support::proto::protorune::BalanceSheet::new()
                    )
                    .entries.len() == 0
            {
                None
            } else {
                Some(v)
            }
        })
        .collect::<Vec<protorune_support::proto::protorune::OutpointResponse>>();
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protoruneholders() -> i32 {
    println!("protoruneholders: Starting function");
    // Log before configuring network
    println!("protoruneholders: About to configure network");
    configure_network();
    println!("protoruneholders: Network configured");
    // Log before getting input data
    println!("protoruneholders: About to get input data");
    let input_data = input();
    println!("protoruneholders: Got input data of length {}", input_data.len());
    // Create cursor and log
    let mut data: Cursor<Vec<u8>> = Cursor::new(input_data);
    println!("protoruneholders: Created cursor");
    // Log before consuming data
    println!("protoruneholders: About to consume data from cursor");
    let consumed_data = match consume_to_end(&mut data) {
        Ok(data) => {
            println!("protoruneholders: Successfully consumed data, length: {}", data.len());
            data
        },
        Err(e) => {
            println!("protoruneholders: ERROR - Failed to consume data: {}", e);
            return -1;
        }
    };

    // Log input data in hex format for debugging
    println!("protoruneholders: Input data in hex: {}", consumed_data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().join(""));
    // Check if the data has a 4-byte prefix and if the 5th byte is 0x0a (which indicates the start of a protobuf message)
    let protobuf_data = if consumed_data.len() > 4 && consumed_data[4] == 0x0a {
        println!("protoruneholders: Detected 4-byte prefix {:02x}{:02x}{:02x}{:02x}, stripping first 4 bytes",
            consumed_data[0], consumed_data[1], consumed_data[2], consumed_data[3]);
        consumed_data[4..].to_vec()
    } else {
        println!("protoruneholders: No prefix detected, using data as is");
        consumed_data
    };
    // Log the processed data
    println!("protoruneholders: Processed data in hex: {}", protobuf_data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().join(""));

    // Try to parse the input as ProtoruneHoldersRequest to see what's in it

    match protorune_support::proto::protorune::ProtoruneHoldersRequest::parse_from_bytes(&protobuf_data) {
        Ok(req) => {
            println!("protoruneholders: Successfully parsed input as ProtoruneHoldersRequest");
            // Log protocol_tag details
            if let Some(protocol_tag) = req.protocol_tag.into_option() {
                println!("protoruneholders: Protocol tag present: lo={}, hi={}", protocol_tag.lo, protocol_tag.hi);
                let protocol_value: u128 = (protocol_tag.hi as u128) << 64 | (protocol_tag.lo as u128);
                println!("protoruneholders: Protocol tag value: {}", protocol_value);
            } else {
                println!("protoruneholders: Protocol tag is NOT present in request");
            }
            // Log height details
            if let Some(height) = req.height.into_option() {
                println!("protoruneholders: Height present: lo={}, hi={}", height.lo, height.hi);
                let height_value: u128 = (height.hi as u128) << 64 | (height.lo as u128);
                println!("protoruneholders: Height value: {}", height_value);
            } else {
                println!("protoruneholders: Height is NOT present in request");
            }
            // Log txindex details
            if let Some(txindex) = req.txindex.into_option() {
                println!("protoruneholders: TxIndex present: lo={}, hi={}", txindex.lo, txindex.hi);
                let txindex_value: u128 = (txindex.hi as u128) << 64 | (txindex.lo as u128);
                println!("protoruneholders: TxIndex value: {}", txindex_value);
            } else {
                println!("protoruneholders: TxIndex is NOT present in request");
            }
        },
        Err(e) => {
            println!("protoruneholders: ERROR - Failed to parse input as ProtoruneHoldersRequest: {}", e);
        }
    }

    // Log before calling protorune_holders

    println!("protoruneholders: About to call protorune_holders function");
    let result: protorune_support::proto::protorune::WalletResponse = match view::protorune_holders(&protobuf_data) {
        Ok(response) => {
            println!("protoruneholders: Successfully got response from protorune_holders");
            println!("protoruneholders: Response contains {} outpoints", response.outpoints.len());
            response
        },

        Err(e) => {
            println!("protoruneholders: ERROR - protorune_holders function failed: {}", e);
            println!("protoruneholders: Error type: {}", std::any::type_name_of_val(&e));
            return -1;
        }
    };


    // Log before serializing result
    println!("protoruneholders: About to serialize result");
    let bytes = match result.write_to_bytes() {
        Ok(bytes) => {
            println!("protoruneholders: Successfully serialized result, length: {}", bytes.len());

            bytes

        },

        Err(e) => {
            println!("protoruneholders: ERROR - Failed to serialize result: {}", e);

            return -1;

        }
    };


    // Log before exporting bytes

    println!("protoruneholders: About to export bytes");
    let result = export_bytes(bytes);
    println!("protoruneholders: Function completed successfully");

    result
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyheight() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse = view
        ::protorunes_by_height(&consume_to_end(&mut data).unwrap())
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
    let outpoint: OutPoint = protorune_support::proto::protorune::Outpoint
        ::parse_from_bytes(&consume_to_end(&mut data).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    export_bytes(view::trace(&outpoint).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyoutpoint() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::OutpointResponse = view
        ::protorunes_by_outpoint(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| protorune_support::proto::protorune::OutpointResponse::new());

    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyheight() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: protorune_support::proto::protorune::RunesResponse = protorune::view
        ::runes_by_height(&consume_to_end(&mut data).unwrap())
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
    let block: Block = consensus_decode::<Block>(
        &mut Cursor::<Vec<u8>>::new(reader.to_vec())
    ).unwrap();

    index_block(&block, height).unwrap();
    flush();
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::message::AlkaneMessageContext;
    use protobuf::{ Message, SpecialFields };
    use protorune::view::{ rune_outpoint_to_outpoint_response, runes_by_address, runes_by_height };
    use protorune::Protorune;
    use protorune_support::proto::protorune::{ RunesByHeightRequest, Uint128, WalletRequest };
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
        let block: Block = consensus_decode::<Block>(
            &mut Cursor::<Vec<u8>>::new(reader.to_vec())
        ).unwrap();
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

        let outpoint_res = rune_outpoint_to_outpoint_response(
            &(OutPoint {
                txid: block.txdata[298].compute_txid(),
                vout: 2,
            })
        ).unwrap();
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
