//! Native standalone binary for the ALKANES indexer.
//!
//! This binary uses the metashrew-lib native runtime to run the ALKANES indexer
//! without requiring a WASM VM, resulting in better performance.

use alkanes::indexer::configure_network;
use alkanes::message::AlkaneMessageContext;
use alkanes::{etl, indexer::index_block};
use anyhow::Result;
use bitcoin::Block;
use metashrew_lib::indexer::{Indexer, NativeIndexer, ProtoViewFunctionWrapper, ViewFunctionWrapper};
use metashrew_lib::native_binary;
use metashrew_lib::view::ProtoViewFunction;
use metashrew_support::block::AuxpowBlock;
use metashrew_support::utils::consensus_decode;
use std::collections::HashMap;
use std::io::Cursor;

/// The ALKANES native indexer
#[derive(Clone)]
struct AlkanesNativeIndexer;

impl Default for AlkanesNativeIndexer {
    fn default() -> Self {
        Self
    }
}

impl Indexer for AlkanesNativeIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        configure_network();
        
        // Parse the block based on the network type
        #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
        let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(block.to_vec()))
            .unwrap()
            .to_consensus();
        #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
        let block: Block = consensus_decode::<Block>(
            &mut Cursor::<Vec<u8>>::new(block.to_vec())
        ).unwrap();

        // Index the block
        index_block(&block, height)?;
        etl::index_extensions(height, &block);
        
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // The actual flush is handled by the metashrew crate
        Ok(Vec::new())
    }
}

// Implement ProtoViewFunction for each view function
impl ProtoViewFunction<alkanes_support::proto::alkanes::MultiSimulateRequest, alkanes_support::proto::alkanes::MultiSimulateResponse> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: alkanes_support::proto::alkanes::MultiSimulateRequest) -> Result<alkanes_support::proto::alkanes::MultiSimulateResponse> {
        configure_network();
        alkanes::view::multi_simulate_safe(
            &alkanes::view::parcels_from_protobuf(request),
            u64::MAX
        ).map(|responses| {
            let mut result = alkanes_support::proto::alkanes::MultiSimulateResponse::new();
            for response in responses {
                let mut res = alkanes_support::proto::alkanes::SimulateResponse::new();
                match response {
                    Ok((response, gas_used)) => {
                        res.execution = protobuf::MessageField::some(response.into());
                        res.gas_used = gas_used;
                    }
                    Err(e) => {
                        result.error = e.to_string();
                    }
                }
                result.responses.push(res);
            }
            result
        })
    }
}

impl ProtoViewFunction<alkanes_support::proto::alkanes::MessageContextParcel, alkanes_support::proto::alkanes::SimulateResponse> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: alkanes_support::proto::alkanes::MessageContextParcel) -> Result<alkanes_support::proto::alkanes::SimulateResponse> {
        configure_network();
        let mut result = alkanes_support::proto::alkanes::SimulateResponse::new();
        match alkanes::view::simulate_safe(&alkanes::view::parcel_from_protobuf(request), u64::MAX) {
            Ok((response, gas_used)) => {
                result.execution = protobuf::MessageField::some(response.into());
                result.gas_used = gas_used;
            }
            Err(e) => {
                result.error = e.to_string();
            }
        }
        Ok(result)
    }
}

impl ProtoViewFunction<alkanes_support::proto::alkanes::MessageContextParcel, Vec<u8>> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: alkanes_support::proto::alkanes::MessageContextParcel) -> Result<Vec<u8>> {
        configure_network();
        alkanes::view::meta_safe(&alkanes::view::parcel_from_protobuf(request))
    }
}

impl ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::WalletResponse> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> Result<protorune_support::proto::protorune::WalletResponse> {
        configure_network();
        protorune::view::runes_by_address(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::OutpointResponse> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> Result<protorune_support::proto::protorune::OutpointResponse> {
        configure_network();
        protorune::view::runes_by_outpoint(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::RunesResponse> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> Result<protorune_support::proto::protorune::RunesResponse> {
        configure_network();
        alkanes::view::protorunes_by_height(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl ProtoViewFunction<u32, Vec<u8>> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: u32) -> Result<Vec<u8>> {
        configure_network();
        alkanes::view::traceblock(request)
    }
}

impl ProtoViewFunction<protorune_support::proto::protorune::Outpoint, Vec<u8>> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: protorune_support::proto::protorune::Outpoint) -> Result<Vec<u8>> {
        configure_network();
        let outpoint: bitcoin::OutPoint = request.try_into().unwrap();
        alkanes::view::trace(&outpoint)
    }
}

impl ProtoViewFunction<Vec<u8>, Vec<u8>> 
    for AlkanesNativeIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> Result<Vec<u8>> {
        configure_network();
        alkanes::view::getbytecode(&request).map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl NativeIndexer for AlkanesNativeIndexer {
    fn view_functions(&self) -> HashMap<String, Box<dyn ViewFunctionWrapper>> {
        let mut map = HashMap::new();
        
        // Add view functions
        map.insert(
            "multisimluate".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, alkanes_support::proto::alkanes::MultiSimulateRequest, alkanes_support::proto::alkanes::MultiSimulateResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "simulate".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, alkanes_support::proto::alkanes::MessageContextParcel, alkanes_support::proto::alkanes::SimulateResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "meta".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, alkanes_support::proto::alkanes::MessageContextParcel, Vec<u8>>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "runesbyaddress".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, protorune_support::proto::protorune::WalletResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "runesbyoutpoint".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, protorune_support::proto::protorune::OutpointResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "protorunesbyheight".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, protorune_support::proto::protorune::RunesResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "traceblock".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, u32, Vec<u8>>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "trace".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, protorune_support::proto::protorune::Outpoint, Vec<u8>>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "getbytecode".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, Vec<u8>>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "protorunesbyoutpoint".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, protorune_support::proto::protorune::OutpointResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "runesbyheight".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, Vec<u8>, protorune_support::proto::protorune::RunesResponse>::new(
                self.clone(),
            )),
        );
        
        map
    }
}

// Define the native binary
native_binary! {
    indexer: AlkanesNativeIndexer,
    name: "alkanes-indexer",
    version: "0.1.0",
    about: "ALKANES metaprotocol indexer",
}