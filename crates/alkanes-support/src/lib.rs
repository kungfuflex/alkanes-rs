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
        Protorune::index_block::<H, AlkaneMessageContext>(block.clone(), height.into())?;

    #[cfg(feature = "cache")]
    {
        // Cache the WalletResponse for each updated address
        for address in updated_addresses.iter() {
            // Skip empty addresses
            if address.is_empty() {
                continue;
            }

            // Create a request for this address
            let mut request = ProtorunesWalletRequest::new();
            request.wallet = address.clone();
            request.protocol_tag = Some(<u128 as Into<
                protorune_support::proto::protorune::Uint128,
            >>::into(AlkaneMessageContext::protocol_tag()))
            .into();

            // Get the WalletResponse for this address (full set of spendable outputs)
            match protorunes_by_address(&request.write_to_bytes()?) {
                Ok(full_response) => {
                    // Cache the serialized full WalletResponse
                    CACHED_WALLET_RESPONSE
                        .select(&address)
                        .set(Arc::new(full_response.write_to_bytes()?));

                    // Create a filtered version with only outpoints that have runes
                    let mut filtered_response = full_response.clone();
                    filtered_response.outpoints = filtered_response
                        .outpoints
                        .into_iter()
                        .filter_map(|v| {
                            if v.balances()
                                .unwrap_or_else(|| {
                                    protorune_support::proto::protorune::BalanceSheet::new()
                                })
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

                    // Cache the serialized filtered WalletResponse
                    CACHED_FILTERED_WALLET_RESPONSE
                        .select(&address)
                        .set(Arc::new(filtered_response.write_to_bytes()?));
                }
                Err(e) => {
                    println!("Error caching wallet response for address: {:?}", e);
                }
            }
        }
    }

    Ok(updated_addresses)
}
