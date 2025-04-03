use crate::message::AlkaneMessageContext;
use crate::network::{genesis, is_genesis};
use crate::vm::fuel::FuelTank;
use anyhow::Result;
use bitcoin::blockdata::block::Block;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::message::MessageContext;
use protorune::Protorune;
use protorune_support::network::{set_network, NetworkParams};

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

pub fn index_block(block: &Block, height: u32) -> Result<()> {
    configure_network();
    let really_is_genesis = is_genesis(height.into());
    if really_is_genesis {
        genesis(&block).unwrap();
    }
    FuelTank::initialize(&block);

    // Get the set of updated addresses from the indexing process
    let updated_addresses =
        Protorune::index_block::<AlkaneMessageContext>(block.clone(), height.into())?;

    #[cfg(feature = "cache")]
    {
        // Cache the WalletResponse for each updated address
        for address in updated_addresses {
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

    Ok(())
}
