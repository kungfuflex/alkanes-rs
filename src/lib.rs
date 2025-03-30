use crate::indexer::configure_network;
use crate::view::{ multi_simulate_safe, parcel_from_protobuf, simulate_safe, meta_safe };
use alkanes_support::proto;
use bitcoin::{ Block, OutPoint };
#[allow(unused_imports)]
use metashrew_core::{ flush, host::log as println, stdio::{ stdout, Write } };
// Import the necessary traits
use metashrew_core::indexer::Indexer;
use metashrew_core::view::ProtoViewFunction;
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::export_bytes;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{ consensus_decode, consume_sized_int, consume_to_end };
use protobuf::{ Message, MessageField };
use std::io::Cursor;
use std::any::Any;
use view::parcels_from_protobuf;
pub mod etl;
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

// Define the AlkanesIndexer struct
#[derive(Clone)]
pub struct AlkanesIndexer;

impl Default for AlkanesIndexer {
    fn default() -> Self {
        Self
    }
}

// Implement the Indexer trait for AlkanesIndexer
impl metashrew_core::indexer::Indexer for AlkanesIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> anyhow::Result<()> {
        configure_network();
        
        #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
        let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(block.to_vec()))
            .unwrap()
            .to_consensus();
        #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
        let block: Block = consensus_decode::<Block>(
            &mut Cursor::<Vec<u8>>::new(block.to_vec())
        ).unwrap();

        index_block(&block, height).unwrap();
        etl::index_extensions(height, &block);
        
        Ok(())
    }
    
    fn flush(&self) -> anyhow::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // The actual flush is handled by the metashrew crate
        Ok(Vec::new())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Add direct method implementations to match the declare_indexer macro's expectations
impl AlkanesIndexer {
    fn multisimluate(&self, request: proto::alkanes::MultiSimulateRequest) -> anyhow::Result<proto::alkanes::MultiSimulateResponse> {
        self.execute_proto(request)
    }
    
    fn simulate(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<proto::alkanes::SimulateResponse> {
        self.execute_proto(request)
    }
    
    fn meta(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<Vec<u8>> {
        self.execute_proto(request)
    }
    
    fn runesbyaddress(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::WalletResponse> {
        self.execute_proto(request)
    }
    
    fn runesbyoutpoint(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
        self.execute_proto(request)
    }
    
    fn protorunesbyheight(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
        self.execute_proto(request)
    }
    
    fn traceblock(&self, request: u32) -> anyhow::Result<Vec<u8>> {
        self.execute_proto(request)
    }
    
    fn trace(&self, request: protorune_support::proto::protorune::Outpoint) -> anyhow::Result<Vec<u8>> {
        self.execute_proto(request)
    }
    
    fn getbytecode(&self, request: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        self.execute_proto(request)
    }
    
    fn protorunesbyoutpoint(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
        self.execute_proto(request)
    }
    
    fn runesbyheight(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
        self.execute_proto(request)
    }
}

// Implement NativeIndexer for AlkanesIndexer
#[cfg(feature = "native")]
impl metashrew_core::indexer::NativeIndexer for AlkanesIndexer {
    fn view_functions(&self) -> std::collections::HashMap<String, Box<dyn metashrew_core::indexer::ViewFunctionWrapper>> {
        use metashrew_core::indexer::ProtoViewFunctionWrapper;
        use metashrew_core::view::ProtoViewFunction;
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        
        // Add view functions
        map.insert(
            "multisimluate".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, proto::alkanes::MultiSimulateRequest, proto::alkanes::MultiSimulateResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "simulate".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, proto::alkanes::MessageContextParcel, proto::alkanes::SimulateResponse>::new(
                self.clone(),
            )),
        );
        
        map.insert(
            "meta".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, proto::alkanes::MessageContextParcel, Vec<u8>>::new(
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

// Implement ProtoViewFunction for each view function
impl metashrew_core::view::ProtoViewFunction<proto::alkanes::MultiSimulateRequest, proto::alkanes::MultiSimulateResponse>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: proto::alkanes::MultiSimulateRequest) -> anyhow::Result<proto::alkanes::MultiSimulateResponse> {
        configure_network();
        let mut result = proto::alkanes::MultiSimulateResponse::new();
        let responses = multi_simulate_safe(
            &parcels_from_protobuf(request),
            u64::MAX
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
        
        Ok(result)
    }
}

impl metashrew_core::view::ProtoViewFunction<proto::alkanes::MessageContextParcel, proto::alkanes::SimulateResponse>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<proto::alkanes::SimulateResponse> {
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
}

impl metashrew_core::view::ProtoViewFunction<proto::alkanes::MessageContextParcel, Vec<u8>>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<Vec<u8>> {
        configure_network();
        meta_safe(&parcel_from_protobuf(request))
    }
}

impl metashrew_core::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::WalletResponse>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::WalletResponse> {
        configure_network();
        protorune::view::runes_by_address(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_core::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::OutpointResponse>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
        configure_network();
        protorune::view::runes_by_outpoint(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_core::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::RunesResponse>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
        configure_network();
        view::protorunes_by_height(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_core::view::ProtoViewFunction<u32, Vec<u8>>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: u32) -> anyhow::Result<Vec<u8>> {
        configure_network();
        view::traceblock(request)
    }
}

impl metashrew_core::view::ProtoViewFunction<protorune_support::proto::protorune::Outpoint, Vec<u8>>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: protorune_support::proto::protorune::Outpoint) -> anyhow::Result<Vec<u8>> {
        configure_network();
        let outpoint: OutPoint = request.try_into().unwrap();
        view::trace(&outpoint)
    }
}

impl metashrew_core::view::ProtoViewFunction<Vec<u8>, Vec<u8>>
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        configure_network();
        view::getbytecode(&request).map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

// Define the WASM exports directly
static mut INDEXER_INSTANCE: Option<metashrew_core::indexer::MetashrewIndexer<AlkanesIndexer>> = None;

#[no_mangle]
pub extern "C" fn _start() {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &mut INDEXER_INSTANCE {
            if let Err(e) = indexer.process_block() {
                metashrew_core::host::log(&format!("Error processing block: {}", e));
            }
        }
    }
}

// Helper function to parse a protobuf message
fn parse_protobuf_message<T: protobuf::Message>(input_bytes: &[u8]) -> Option<T> {
    T::parse_from_bytes(input_bytes).ok()
}

// Helper function to serialize a protobuf message
fn serialize_protobuf_message<T: protobuf::Message>(message: &T) -> Vec<u8> {
    message.write_to_bytes().unwrap_or_else(|_| Vec::new())
}

// Define view functions
macro_rules! define_view_function {
    ($name:ident, $request_type:ty, $response_type:ty) => {
        #[no_mangle]
        pub extern "C" fn $name() -> i32 {
            unsafe {
                if INDEXER_INSTANCE.is_none() {
                    let indexer = AlkanesIndexer::default();
                    INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
                }

                if let Some(indexer) = &INDEXER_INSTANCE {
                    // Load the input data
                    let (_height, input_bytes) = match metashrew_core::host::load_input() {
                        Ok(input) => input,
                        Err(e) => {
                            metashrew_core::host::log(&format!("Error loading input: {}", e));
                            return metashrew_core::view::return_view_result(&[]);
                        }
                    };

                    // Parse the request based on its type
                    let request: $request_type = if std::any::TypeId::of::<$request_type>() == std::any::TypeId::of::<Vec<u8>>() {
                        // For Vec<u8>, just use the input bytes directly
                        input_bytes.clone()
                    } else if std::any::TypeId::of::<$request_type>() == std::any::TypeId::of::<u32>() {
                        // For u32, convert from bytes
                        if input_bytes.len() >= 4 {
                            let mut bytes = [0u8; 4];
                            bytes.copy_from_slice(&input_bytes[0..4]);
                            u32::from_le_bytes(bytes)
                        } else {
                            metashrew_core::host::log("Error: input bytes too short for u32");
                            return metashrew_core::view::return_view_result(&[]);
                        }
                    } else {
                        // For Protocol Buffer types, use parse_from_bytes
                        match parse_protobuf_message(&input_bytes) {
                            Some(req) => req,
                            None => {
                                // If parsing fails, create a default instance
                                Default::default()
                            }
                        }
                    };

                    // Call the view function
                    match indexer.get_indexer().$name(request) {
                        Ok(response) => {
                            // Serialize the response based on its type
                            let bytes = if std::any::TypeId::of::<$response_type>() == std::any::TypeId::of::<Vec<u8>>() {
                                // For Vec<u8>, just use the bytes directly
                                response
                            } else if std::any::TypeId::of::<$response_type>() == std::any::TypeId::of::<u32>() {
                                // For u32, convert to bytes
                                let value: u32 = response;
                                value.to_le_bytes().to_vec()
                            } else {
                                // For Protocol Buffer types, use write_to_bytes
                                match serialize_protobuf_message(&response) {
                                    bytes if !bytes.is_empty() => bytes,
                                    _ => {
                                        // If serialization fails, just convert to a string representation
                                        format!("{:?}", response).into_bytes()
                                    }
                                }
                            };

                            metashrew_core::view::return_view_result(&bytes)
                        },
                        Err(e) => {
                            metashrew_core::host::log(&format!("Error executing view function: {}", e));
                            metashrew_core::view::return_view_result(&[])
                        }
                    }
                } else {
                    metashrew_core::view::return_view_result(&[])
                }
            }
        }
    };
}

// Define the view functions
define_view_function!(multisimluate, proto::alkanes::MultiSimulateRequest, proto::alkanes::MultiSimulateResponse);
define_view_function!(simulate, proto::alkanes::MessageContextParcel, proto::alkanes::SimulateResponse);
define_view_function!(meta, proto::alkanes::MessageContextParcel, Vec<u8>);
define_view_function!(runesbyaddress, Vec<u8>, protorune_support::proto::protorune::WalletResponse);
define_view_function!(runesbyoutpoint, Vec<u8>, protorune_support::proto::protorune::OutpointResponse);
define_view_function!(protorunesbyheight, Vec<u8>, protorune_support::proto::protorune::RunesResponse);
define_view_function!(traceblock, u32, Vec<u8>);
define_view_function!(trace, protorune_support::proto::protorune::Outpoint, Vec<u8>);
define_view_function!(getbytecode, Vec<u8>, Vec<u8>);
define_view_function!(protorunesbyoutpoint, Vec<u8>, protorune_support::proto::protorune::OutpointResponse);
define_view_function!(runesbyheight, Vec<u8>, protorune_support::proto::protorune::RunesResponse);

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
}