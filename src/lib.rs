use crate::indexer::{configure_network, index_block};
use crate::view::{
    meta_safe, multi_simulate_safe, parcel_from_protobuf, parcels_from_protobuf, simulate_safe,
};
use alkanes_support::proto;
use bitcoin::consensus::Decodable;
use bitcoin::{Block, OutPoint};
use metashrew_core::declare_indexer;
#[allow(unused_imports)]
use metashrew_core::println;
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::utils::{consume_sized_int, consume_to_end};
use protobuf::MessageField;
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
pub mod utils;
pub mod view;
pub mod vm;

// Define the AlkanesIndexer struct
pub struct AlkanesIndexer;

declare_indexer! {
    impl AlkanesIndexer {
        fn index_block(height: u32, block: bitcoin::Block) {
            indexer::index_block(&block, height).unwrap();
        }

        #[view]
        fn multisimulate(request: proto::alkanes::MultiSimulateRequest) -> Result<proto::alkanes::MultiSimulateResponse, String> {
            configure_network();
            let mut result = proto::alkanes::MultiSimulateResponse::new();
            let responses = multi_simulate_safe(&parcels_from_protobuf(request), u64::MAX);

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

            Ok(result)
        }

        #[view]
        fn simulate(request: proto::alkanes::MessageContextParcel) -> Result<proto::alkanes::SimulateResponse, String> {
            configure_network();
            let mut result = proto::alkanes::SimulateResponse::new();
            match simulate_safe(&parcel_from_protobuf(request), u64::MAX) {
                Ok((response, gas_used)) => {
                    result.execution = MessageField::some(response.into());
                    result.gas_used = gas_used;
                }
                Err(e) => {
                    result.error = e.to_string();
                }
            }
            Ok(result)
        }

        #[view]
        fn meta(request: proto::alkanes::MessageContextParcel) -> Result<Vec<u8>, String> {
            configure_network();
            meta_safe(&parcel_from_protobuf(request)).map_err(|e| e.to_string())
        }

        #[view]
        fn runesbyaddress(request: protorune_support::proto::protorune::WalletRequest) -> Result<protorune_support::proto::protorune::WalletResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            protorune::view::runes_by_address(&request_bytes)
                .map_err(|e| e.to_string())
                .or_else(|_| Ok(protorune_support::proto::protorune::WalletResponse::new()))
        }

        #[view]
        fn runesbyoutpoint(request: protorune_support::proto::protorune::Outpoint) -> Result<protorune_support::proto::protorune::OutpointResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            protorune::view::runes_by_outpoint(&request_bytes)
                .map_err(|e| e.to_string())
                .or_else(|_| Ok(protorune_support::proto::protorune::OutpointResponse::new()))
        }

        #[view]
        fn protorunesbyheight(request: protorune_support::proto::protorune::ProtorunesByHeightRequest) -> Result<protorune_support::proto::protorune::RunesResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            view::protorunes_by_height(&request_bytes)
                .map_err(|e| e.to_string())
                .or_else(|_| Ok(protorune_support::proto::protorune::RunesResponse::new()))
        }

        #[view]
        fn traceblock(request: proto::alkanes::TraceBlockRequest) -> Result<proto::alkanes::AlkanesBlockTraceEvent, String> {
            configure_network();
            let height = request.height;
            let trace_bytes = view::traceblock(height).map_err(|e| e.to_string())?;
            proto::alkanes::AlkanesBlockTraceEvent::parse_from_bytes(&trace_bytes)
                .map_err(|e| e.to_string())
        }

        #[view]
        fn trace(request: protorune_support::proto::protorune::Outpoint) -> Result<proto::alkanes::AlkanesTrace, String> {
            configure_network();
            let outpoint: OutPoint = request.try_into().map_err(|_| "Failed to convert outpoint".to_string())?;
            let trace_bytes = view::trace(&outpoint).map_err(|e| e.to_string())?;
            proto::alkanes::AlkanesTrace::parse_from_bytes(&trace_bytes)
                .map_err(|e| e.to_string())
        }

        #[view]
        fn getbytecode(request: proto::alkanes::BytecodeRequest) -> Result<Vec<u8>, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            match view::getbytecode(&request_bytes) {
                Ok(bytes) => Ok(bytes),
                Err(_) => Ok(Vec::new()) // Return empty vector on error
            }
        }

        #[view]
        fn protorunesbyoutpoint(request: protorune_support::proto::protorune::OutpointWithProtocol) -> Result<protorune_support::proto::protorune::OutpointResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            view::protorunes_by_outpoint(&request_bytes)
                .map_err(|e| e.to_string())
                .or_else(|_| Ok(protorune_support::proto::protorune::OutpointResponse::new()))
        }

        #[view]
        fn runesbyheight(request: protorune_support::proto::protorune::RunesByHeightRequest) -> Result<protorune_support::proto::protorune::RunesResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            protorune::view::runes_by_height(&request_bytes)
                .map_err(|e| e.to_string())
                .or_else(|_| Ok(protorune_support::proto::protorune::RunesResponse::new()))
        }

        #[view]
        fn getblock(request: proto::alkanes::BlockRequest) -> Result<proto::alkanes::BlockResponse, String> {
            configure_network();
            let request_bytes = request.write_to_bytes().map_err(|e| e.to_string())?;
            view::getblock(&request_bytes)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    proto::alkanes::BlockResponse::parse_from_bytes(&bytes)
                        .map_err(|e| e.to_string())
                })
        }
    }
}
