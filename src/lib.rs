use anyhow::Result;
use bitcoin::Block;
use metashrew_core::{declare_indexer, flush, input, println, stdio::{stdout, Write}};
use metashrew_support::block::AuxpowBlock;
use metashrew_support::utils::consensus_decode;
use protobuf::Message;
use std::io::Cursor;
use std::any::Any;

// Module exports
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

// Import the indexer function
use crate::indexer::{configure_network, index_block};
use crate::view::{
    multi_simulate_safe, parcel_from_protobuf, parcels_from_protobuf, simulate_safe, meta_safe,
    protorunes_by_address, protorunes_by_height, protorunes_by_outpoint, traceblock, trace, getbytecode
};

// Define the AlkanesIndexer struct
#[derive(Default)]
pub struct AlkanesIndexer;

// Implement the Indexer trait for AlkanesIndexer
impl metashrew_core::indexer::Indexer for AlkanesIndexer {
    fn index_block(&mut self, height: u32, block_data: &[u8]) -> Result<()> {
        // Configure the network
        configure_network();
        
        // Parse the block data
        #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
        let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(block_data.to_vec()))
            .unwrap()
            .to_consensus();
        #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
        let block: Block = consensus_decode::<Block>(
            &mut Cursor::<Vec<u8>>::new(block_data.to_vec())
        ).unwrap();

        // Process the block using the alkanes indexer
        index_block(&block, height)?;
        
        // Process any extensions
        etl::index_extensions(height, &block);
        
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // In the original implementation, this was handled by the metashrew flush() function
        // We'll return an empty vector for now
        Ok(Vec::new())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Use the declare_indexer! macro to define the indexer and its view functions
declare_indexer! {
    struct AlkanesIndexerProgram {
        indexer: AlkanesIndexer,
        views: {
            "multisimulate" => {
                fn multisimulate(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let mut result = alkanes_support::proto::alkanes::MultiSimulateResponse::new();
                    let responses = multi_simulate_safe(
                        &parcels_from_protobuf(
                            alkanes_support::proto::alkanes::MultiSimulateRequest::parse_from_bytes(&request)?
                        ),
                        u64::MAX
                    );

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

                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "simulate" => {
                fn simulate(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let mut result = alkanes_support::proto::alkanes::SimulateResponse::new();
                    match simulate_safe(
                        &parcel_from_protobuf(
                            alkanes_support::proto::alkanes::MessageContextParcel::parse_from_bytes(&request)?
                        ),
                        u64::MAX
                    ) {
                        Ok((response, gas_used)) => {
                            result.execution = protobuf::MessageField::some(response.into());
                            result.gas_used = gas_used;
                        }
                        Err(e) => {
                            result.error = e.to_string();
                        }
                    }
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "meta" => {
                fn meta(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    meta_safe(
                        &parcel_from_protobuf(
                            alkanes_support::proto::alkanes::MessageContextParcel::parse_from_bytes(&request)?
                        )
                    )
                }
            },
            "runesbyaddress" => {
                fn runesbyaddress(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let result = protorune::view::runes_by_address(&request)?;
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "runesbyoutpoint" => {
                fn runesbyoutpoint(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let result = protorune::view::runes_by_outpoint(&request)?;
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "protorunesbyheight" => {
                fn protorunesbyheight(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let result = protorunes_by_height(&request)?;
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "traceblock" => {
                fn traceblock(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let height = u32::from_le_bytes((&request[0..4]).try_into()?);
                    view::traceblock(height)
                }
            },
            "trace" => {
                fn trace(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let outpoint: bitcoin::OutPoint = protorune_support::proto::protorune::Outpoint
                        ::parse_from_bytes(&request)?
                        .try_into()?;
                    view::trace(&outpoint)
                }
            },
            "getbytecode" => {
                fn getbytecode(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    view::getbytecode(&request)
                }
            },
            "protorunesbyoutpoint" => {
                fn protorunesbyoutpoint(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let result = protorunes_by_outpoint(&request)?;
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
            "runesbyheight" => {
                fn runesbyheight(&self, request: Vec<u8>) -> Result<Vec<u8>> {
                    configure_network();
                    let result = protorune::view::runes_by_height(&request)?;
                    result.write_to_bytes().map_err(|e| anyhow::anyhow!("{:?}", e))
                }
            },
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
