use crate::message::AlkaneMessageContext;
use crate::network::{genesis, genesis_alkane_upgrade_bytes, is_genesis};
use crate::unwrap::{fr_btc_payments_at_block, fr_btc_storage_pointer, deserialize_payments};
use crate::vm::fuel::FuelTank;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::blockdata::block::Block;
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
#[allow(unused_imports)]
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::Protorune;
use protorune::tables::OUTPOINT_SPENDABLE_BY;
use protorune_support::network::{set_network, NetworkParams};
use std::sync::Arc;
use metashrew_support::utils::consensus_encode;

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

pub fn index_cleanup(height: u32) -> Result<()> {
    let mut last_block_ptr = fr_btc_storage_pointer().select(&b"/last_block".to_vec());
    let mut last_block = last_block_ptr.get_value();
    for i in last_block..=height as u128 {
        let payments = fr_btc_payments_at_block(i);
        let mut all_spent = true;
        for payment_list_bytes in payments {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                if OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get().len() > 0 {
                    all_spent = false;
                    break;
                }
            }
            if !all_spent {
                break;
            }
        }
        if all_spent {
            last_block = i;
        }
    }
    last_block_ptr.set_value(last_block);
    Ok(())
}

pub fn index_block(block: &Block, height: u32) -> Result<()> {
    configure_network();
    let really_is_genesis = is_genesis(height.into());
    if really_is_genesis {
        genesis(&block).unwrap();
    }
    if height >= genesis::GENESIS_UPGRADE_BLOCK_HEIGHT {
        let mut upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded");
        if upgrade_ptr.get().len() == 0 {
            upgrade_ptr.set_value::<u8>(0x01);
            IndexPointer::from_keyword("/alkanes/")
                .select(&AlkaneId { block: 2, tx: 0 }.into())
                .set(Arc::new(compress(genesis_alkane_upgrade_bytes())?));
        }
    }
    FuelTank::initialize(&block, height);

    // Get the set of updated addresses from the indexing process
    let _updated_addresses =
        Protorune::index_block::<AlkaneMessageContext>(block.clone(), height.into())?;

    #[cfg(feature = "cache")]
    {
        // Cache the WalletResponse for each updated address
        for address in _updated_addresses {
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
            match protorunes_by_address(&request.encode_to_vec()?) {
                Ok(full_response) => {
                    // Cache the serialized full WalletResponse
                    CACHED_WALLET_RESPONSE
                        .select(&address)
                        .set(Arc::new(full_response.encode_to_vec()?));

                    // Create a filtered version with only outpoints that have runes
                    let mut filtered_response = full_response.clone();
                    filtered_response.outpoints = filtered_response
                        .outpoints
                        .into_iter()
                        .filter_map(|v| {
                            if v.clone()
                                .balances
                                .unwrap_or_else(|| {
                                    protorune_support::proto::protorune::BalanceSheet::default()
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
                        .set(Arc::new(filtered_response.encode_to_vec()?));
                }
                Err(e) => {
                    println!("Error caching wallet response for address: {:?}", e);
                }
            }
        }
    }
    index_cleanup(height)?;
    Ok(())
}
