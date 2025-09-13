pub mod host;
pub mod network;
pub mod message;
use anyhow::Result;
use bitcoin::blockdata::block::Block;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
#[allow(unused_imports)]
use metashrew_support::index_pointer::KeyValuePointer;
use crate::message::AlkaneMessageContext;
use crate::network::Network;
use protorune_support::network::set_network;
use protorune_support::Protorune;
use std::collections::BTreeSet;

pub fn configure_network(network: Network) {
    set_network(network.default_params());
}

#[cfg(feature = "cache")]
use crate::view::protorunes_by_address;
#[cfg(feature = "cache")]
use protobuf::{Message, MessageField};
#[cfg(feature = "cache")]
use protorune::tables::{CACHED_FILTERED_WALLET_RESPONSE, CACHED_WALLET_RESPONSE};
#[cfg(feature = "cache")]
use protorune_support::proto::protorune::ProtorunesWalletRequest;
#[cfg(feature = "cache")]
use std::sync::Arc;

use crate::host::AlkanesHost;

use metashrew_core::index_pointer::AtomicPointer;

pub fn index_block<
    H: AlkanesHost + protorune_support::host::Host<Pointer = AtomicPointer> + Default,
    T: protorune_support::message::MessageContext<H>,
>(
    host: &H,
    block: &Block,
    height: u32,
    network: Network,
) -> Result<BTreeSet<Vec<u8>>>
where
    <H as protorune_support::host::Host>::Pointer: Default + Clone,
{
    configure_network(network);
    // Get the set of updated addresses from the indexing process
    let updated_addresses =
        Protorune::index_block::<H, T>(block.clone(), height.into())?;

    Ok(updated_addresses)
}
