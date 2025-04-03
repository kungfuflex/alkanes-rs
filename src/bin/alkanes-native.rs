use alkanes::indexer::{configure_network, index_block};
use alkanes::message::AlkaneMessageContext;
use alkanes::view::{
    meta_safe, multi_simulate_safe, parcel_from_protobuf, parcels_from_protobuf, protorunes_by_address,
    protorunes_by_height, protorunes_by_outpoint, simulate_safe, traceblock, trace, getbytecode,
    runesbyaddress, runesbyoutpoint, runesbyheight,
};
use alkanes_support::proto;
use anyhow::{anyhow, Result};
use metashrew_core::{indexer::{Indexer, KeyValueStore, NativeIndexer, ProtoViewFunctionWrapper, ViewFunctionWrapper}, native_binary};
use protobuf::{Message, MessageField};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

// Define the AlkanesIndexer struct
#[derive(Clone)]
struct AlkanesIndexer {
    store: KeyValueStore,
}

// Implement Default for AlkanesIndexer
impl Default for AlkanesIndexer {
    fn default() -> Self {
        configure_network();
        Self {
            store: KeyValueStore::new(),
        }
    }
}

// Implement Indexer trait for AlkanesIndexer
impl Indexer for AlkanesIndexer {
    fn index_block(&mut self, height: u32, block_data: &[u8]) -> Result<()> {
        // Parse the block data
        #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
        let block: bitcoin::Block = metashrew_support::block::AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(block_data.to_vec()))
            .unwrap()
            .to_consensus();
        #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
        let block: bitcoin::Block = metashrew_support::utils::consensus_decode::<bitcoin::Block>(
            &mut Cursor::<Vec<u8>>::new(block_data.to_vec())
        ).unwrap();

        // Process the block using the alkanes indexer
        index_block(&block, height)?;
        
        // Process any extensions
        alkanes::etl::index_extensions(height, &block);
        
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // Return the key-value pairs from the store
        Ok(self.store.pairs())
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Implement view functions for AlkanesIndexer
impl AlkanesIndexer {
    // Simulate function
    fn simulate(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let parcel = parcel_from_protobuf(
            proto::alkanes::MessageContextParcel::parse_from_bytes(&input)?
        );
        
        let mut result: proto::alkanes::SimulateResponse = proto::alkanes::SimulateResponse::new();
        match simulate_safe(&parcel, u64::MAX) {
            Ok((response, gas_used)) => {
                result.execution = MessageField::some(response.into());
                result.gas_used = gas_used;
            }
            Err(e) => {
                result.error = e.to_string();
            }
        }
        
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Multi-simulate function
    fn multisimulate(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let parcels = parcels_from_protobuf(
            proto::alkanes::MultiSimulateRequest::parse_from_bytes(&input)?
        );
        
        let mut result: proto::alkanes::MultiSimulateResponse = proto::alkanes::MultiSimulateResponse::new();
        let responses = multi_simulate_safe(&parcels, u64::MAX);
        
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
        
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Meta function
    fn meta(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let parcel = parcel_from_protobuf(
            proto::alkanes::MessageContextParcel::parse_from_bytes(&input)?
        );
        
        meta_safe(&parcel).map_err(|e| anyhow!("{:?}", e))
    }
    
    // Protorunes by address function
    fn protorunes_by_address(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = protorunes_by_address(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Protorunes by outpoint function
    fn protorunes_by_outpoint(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = protorunes_by_outpoint(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Protorunes by height function
    fn protorunes_by_height(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = protorunes_by_height(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Runes by address function
    fn runes_by_address(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = runesbyaddress(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Runes by outpoint function
    fn runes_by_outpoint(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = runesbyoutpoint(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Runes by height function
    fn runes_by_height(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let result = runesbyheight(&input)?;
        result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
    }
    
    // Trace block function
    fn trace_block(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let height = u32::from_le_bytes((&input[0..4]).try_into()?);
        traceblock(height)
    }
    
    // Trace function
    fn trace_tx(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let outpoint: bitcoin::OutPoint = protorune_support::proto::protorune::Outpoint
            ::parse_from_bytes(&input)?
            .try_into()?;
        trace(&outpoint)
    }
    
    // Get bytecode function
    fn get_bytecode(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        getbytecode(&input)
    }
}

// Implement ProtoViewFunction for each view function
struct SimulateView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for SimulateView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().simulate(input)
    }
}

struct MultiSimulateView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for MultiSimulateView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().multisimulate(input)
    }
}

struct MetaView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for MetaView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().meta(input)
    }
}

struct ProtorunesByAddressView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for ProtorunesByAddressView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().protorunes_by_address(input)
    }
}

struct ProtorunesByOutpointView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for ProtorunesByOutpointView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().protorunes_by_outpoint(input)
    }
}

struct ProtorunesByHeightView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for ProtorunesByHeightView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().protorunes_by_height(input)
    }
}

struct RunesByAddressView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for RunesByAddressView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().runes_by_address(input)
    }
}

struct RunesByOutpointView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for RunesByOutpointView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().runes_by_outpoint(input)
    }
}

struct RunesByHeightView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for RunesByHeightView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().runes_by_height(input)
    }
}

struct TraceBlockView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for TraceBlockView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().trace_block(input)
    }
}

struct TraceTxView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for TraceTxView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().trace_tx(input)
    }
}

struct GetBytecodeView(Arc<Mutex<AlkanesIndexer>>);
impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>> for GetBytecodeView {
    fn execute_proto(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        self.0.lock().unwrap().get_bytecode(input)
    }
}

// Implement NativeIndexer trait for AlkanesIndexer
impl NativeIndexer for AlkanesIndexer {
    fn view_functions(&self) -> HashMap<String, Box<dyn ViewFunctionWrapper>> {
        let indexer = Arc::new(Mutex::new(self.clone()));
        
        let mut map = HashMap::new();
        
        // Add view functions
        map.insert(
            "simulate".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(SimulateView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "multisimulate".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(MultiSimulateView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "meta".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(MetaView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "protorunesbyaddress".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(ProtorunesByAddressView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "protorunesbyoutpoint".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(ProtorunesByOutpointView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "protorunesbyheight".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(ProtorunesByHeightView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "runesbyaddress".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(RunesByAddressView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "runesbyoutpoint".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(RunesByOutpointView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "runesbyheight".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(RunesByHeightView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "traceblock".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(TraceBlockView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "trace".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(TraceTxView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map.insert(
            "getbytecode".to_string(),
            Box::new(ProtoViewFunctionWrapper::new(GetBytecodeView(indexer.clone()))) as Box<dyn ViewFunctionWrapper>
        );
        
        map
    }
}

// Use the native_binary! macro to create the native binary
native_binary! {
    indexer: AlkanesIndexer,
    name: "alkanes-native",
    version: env!("CARGO_PKG_VERSION"),
    about: "ALKANES metaprotocol indexer",
}