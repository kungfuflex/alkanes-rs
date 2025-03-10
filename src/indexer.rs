use crate::message::AlkaneMessageContext;
use crate::network::{genesis, is_genesis};
use crate::vm::fuel::FuelTank;
use anyhow::Result;
use bitcoin::blockdata::block::Block;
#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};
use protorune::Protorune;
use protorune_support::network::{set_network, NetworkParams};
use crate::tables::BLOCK_TRACES_CACHE;
use crate::tables::BLOCK_TRACES;
use std::sync::Arc;
use metashrew_support::index_pointer::KeyValuePointer;

#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "testnet"),
    not(feature = "luckycoin"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin")
))]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bcrt"),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });
}
#[cfg(feature = "mainnet")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bc"),
        p2sh_prefix: 0x05,
        p2pkh_prefix: 0x00,
    });
}
#[cfg(feature = "testnet")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("tb"),
        p2pkh_prefix: 0x6f,
        p2sh_prefix: 0xc4,
    });
}
#[cfg(feature = "luckycoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("lky"),
        p2pkh_prefix: 0x2f,
        p2sh_prefix: 0x05,
    });
}

#[cfg(feature = "dogecoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("dc"),
        p2pkh_prefix: 0x1e,
        p2sh_prefix: 0x16,
    });
}
#[cfg(feature = "bellscoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bel"),
        p2pkh_hash: 0x19,
        p2sh_hash: 0x1e,
    });
}

pub fn index_block(block: &Block, height: u32) -> Result<()> {
    // Reset the BlockTrace cache at the beginning of each block
    BLOCK_TRACES_CACHE.write().unwrap().clear();
    
    configure_network();
    let really_is_genesis = is_genesis(height.into());
    if really_is_genesis {
        genesis(&block).unwrap();
    }
    FuelTank::initialize(&block);
    Protorune::index_block::<AlkaneMessageContext>(block.clone(), height.into())?;
    
    // Save the complete BlockTrace to persistent storage
    save_trace_block(height.into())?;
    
    Ok(())
}

// Function to save the complete BlockTrace to persistent storage
fn save_trace_block(height: u64) -> Result<()> {
    // Get the cached BlockTrace
    let cached_bytes = {
        let cache = BLOCK_TRACES_CACHE.read().unwrap();
        if let Some(bytes) = cache.get(&height) {
            bytes.clone()
        } else {
            // If no events were processed for this block, create an empty BlockTrace
            use alkanes_support::proto::alkanes::AlkanesBlockTraceEvent;
            use protobuf::Message;
            
            let empty_trace = AlkanesBlockTraceEvent::new();
            empty_trace.write_to_bytes()?
        }
    };
    
    // Store in the persistent BLOCK_TRACES table
    BLOCK_TRACES.select_value::<u64>(height).set(Arc::new(cached_bytes));
    
    Ok(())
}
