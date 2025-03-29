use crate::indexer::configure_network;
use crate::view::{ multi_simulate_safe, parcel_from_protobuf, simulate_safe, meta_safe };
use alkanes_support::proto;
use bitcoin::{ Block, OutPoint };
#[allow(unused_imports)]
use metashrew_lib::{ flush, host::log as println, stdio::{ stdout, Write } };
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::export_bytes;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{ consensus_decode, consume_sized_int, consume_to_end };
use protobuf::{ Message, MessageField };
use std::io::Cursor;
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

// Import the metashrew-lib macros
use metashrew_lib::declare_indexer;

// Define the AlkanesIndexer struct
#[derive(Clone)]
pub struct AlkanesIndexer;

impl Default for AlkanesIndexer {
    fn default() -> Self {
        Self
    }
}

// Implement the Indexer trait for AlkanesIndexer
impl metashrew_lib::indexer::Indexer for AlkanesIndexer {
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
}

// Implement NativeIndexer for AlkanesIndexer
#[cfg(feature = "native")]
impl metashrew_lib::indexer::NativeIndexer for AlkanesIndexer {
    fn view_functions(&self) -> std::collections::HashMap<String, Box<dyn metashrew_lib::indexer::ViewFunctionWrapper>> {
        use metashrew_lib::indexer::ProtoViewFunctionWrapper;
        use metashrew_lib::view::ProtoViewFunction;
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
impl metashrew_lib::view::ProtoViewFunction<proto::alkanes::MultiSimulateRequest, proto::alkanes::MultiSimulateResponse> 
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

impl metashrew_lib::view::ProtoViewFunction<proto::alkanes::MessageContextParcel, proto::alkanes::SimulateResponse> 
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

impl metashrew_lib::view::ProtoViewFunction<proto::alkanes::MessageContextParcel, Vec<u8>> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<Vec<u8>> {
        configure_network();
        meta_safe(&parcel_from_protobuf(request))
    }
}

impl metashrew_lib::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::WalletResponse> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::WalletResponse> {
        configure_network();
        protorune::view::runes_by_address(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_lib::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::OutpointResponse> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
        configure_network();
        protorune::view::runes_by_outpoint(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_lib::view::ProtoViewFunction<Vec<u8>, protorune_support::proto::protorune::RunesResponse> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
        configure_network();
        view::protorunes_by_height(&request)
            .map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

impl metashrew_lib::view::ProtoViewFunction<u32, Vec<u8>> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: u32) -> anyhow::Result<Vec<u8>> {
        configure_network();
        view::traceblock(request)
    }
}

impl metashrew_lib::view::ProtoViewFunction<protorune_support::proto::protorune::Outpoint, Vec<u8>> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: protorune_support::proto::protorune::Outpoint) -> anyhow::Result<Vec<u8>> {
        configure_network();
        let outpoint: OutPoint = request.try_into().unwrap();
        view::trace(&outpoint)
    }
}

impl metashrew_lib::view::ProtoViewFunction<Vec<u8>, Vec<u8>> 
    for AlkanesIndexer 
{
    fn execute_proto(&self, request: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        configure_network();
        view::getbytecode(&request).map_err(|e| anyhow::anyhow!("Error: {:?}", e))
    }
}

// Define the Metashrew indexer program with Protocol Buffer messages
declare_indexer! {
    struct AlkanesProgram {
        indexer: AlkanesIndexer,
        views: {
            "multisimluate" => {
                fn multisimluate(&self, request: proto::alkanes::MultiSimulateRequest) -> anyhow::Result<proto::alkanes::MultiSimulateResponse> {
                    self.execute_proto(request)
                }
            },
            "simulate" => {
                fn simulate(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<proto::alkanes::SimulateResponse> {
                    self.execute_proto(request)
                }
            },
            "meta" => {
                fn meta(&self, request: proto::alkanes::MessageContextParcel) -> anyhow::Result<Vec<u8>> {
                    self.execute_proto(request)
                }
            },
            "runesbyaddress" => {
                fn runesbyaddress(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::WalletResponse> {
                    self.execute_proto(request)
                }
            },
            "runesbyoutpoint" => {
                fn runesbyoutpoint(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
                    self.execute_proto(request)
                }
            },
            "protorunesbyheight" => {
                fn protorunesbyheight(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
                    self.execute_proto(request)
                }
            },
            "traceblock" => {
                fn traceblock(&self, request: u32) -> anyhow::Result<Vec<u8>> {
                    self.execute_proto(request)
                }
            },
            "trace" => {
                fn trace(&self, request: protorune_support::proto::protorune::Outpoint) -> anyhow::Result<Vec<u8>> {
                    self.execute_proto(request)
                }
            },
            "getbytecode" => {
                fn getbytecode(&self, request: Vec<u8>) -> anyhow::Result<Vec<u8>> {
                    self.execute_proto(request)
                }
            },
            "protorunesbyoutpoint" => {
                fn protorunesbyoutpoint(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::OutpointResponse> {
                    self.execute_proto(request)
                }
            },
            "runesbyheight" => {
                fn runesbyheight(&self, request: Vec<u8>) -> anyhow::Result<protorune_support::proto::protorune::RunesResponse> {
                    self.execute_proto(request)
                }
            }
        }
    }
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
