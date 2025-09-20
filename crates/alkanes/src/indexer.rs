use alkanes_support::logging;
use metashrew_support::{
    environment::RuntimeEnvironment,
    index_pointer::{IndexPointer, KeyValuePointer},
};
use protorune::{
    balance_sheet::{BalanceSheetOperations, PersistentRecord},
    message::MessageContext,
};
use crate::message::AlkaneMessageContext;
use crate::network::{
    check_and_upgrade_diesel, genesis, is_genesis, setup_diesel, setup_frbtc, setup_frsigil,
};
use crate::unwrap;
use crate::vm::fuel::FuelTank;
use crate::vm::host_functions::clear_diesel_mints_cache;
use anyhow::Result;
use bitcoin::blockdata::block::Block;
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
        p2pkh_prefix: 0x19,
        p2sh_prefix: 0x1e,
    });
}

#[cfg(feature = "cache")]
use crate::view::protorunes_by_address;
#[cfg(feature = "cache")]
use metashrew_core::app::Initialise;
#[cfg(feature = "cache")]
use prost::Message;
#[cfg(feature = "cache")]
use protorune::tables::{CACHED_FILTERED_WALLET_RESPONSE, CACHED_WALLET_RESPONSE};
#[cfg(feature = "cache")]
use protorune_support::proto::protorune::ProtorunesWalletRequest;
#[cfg(feature = "cache")]
use std::sync::Arc;


pub fn index_block<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    block: &Block,
    height: u32,
) -> Result<()> {
    logging::init_block_stats();
    logging::record_transactions(block.txdata.len() as u32);
    configure_network();
    clear_diesel_mints_cache();
    let really_is_genesis = is_genesis(env, height.into());
    if really_is_genesis {
        genesis(env).unwrap();
        let mut genesis_balance_sheet = setup_diesel(env, block)?;
        let frbtc_balance_sheet = setup_frbtc(env, block)?;
        let frsigil_balance_sheet = setup_frsigil(env, block)?;
        genesis_balance_sheet.merge_sheets(&frbtc_balance_sheet, &frsigil_balance_sheet, env)?;
        let outpoint_bytes = protorune_support::utils::outpoint_encode(&bitcoin::OutPoint {
            txid: protorune_support::utils::tx_hex_to_txid(crate::network::genesis::GENESIS_OUTPOINT)?,
            vout: 0,
        })?;
        let mut atomic = metashrew_support::index_pointer::AtomicPointer::default();
        genesis_balance_sheet.save(
            &mut atomic.derive(
                &protorune::tables::RuneTable::for_protocol(
                    AlkaneMessageContext::<E>::protocol_tag(),
                )
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
            ),
            false,
            env,
        );
        atomic.commit(env);
    }
    check_and_upgrade_diesel(env, height)?;
    FuelTank::initialize::<E>(&block, height);
    // Get the set of updated addresses from the indexing process
    let _updated_addresses = Protorune::index_block::<AlkaneMessageContext<E>>(env, block.clone(), height.into())?;

    let _ = unwrap::update_last_block(env, height as u128)?;

    #[cfg(feature = "cache")]
    {
        // Cache the WalletResponse for each updated address
        for address in _updated_addresses {
            // Skip empty addresses
            if address.is_empty() {
                continue;
            }

            // Create a request for this address
            let mut request = ProtorunesWalletRequest::default();
            request.wallet = address.clone();
            request.protocol_tag = Some(<u128 as Into<
                protorune_support::proto::protorune::Uint128,
            >>::into(Protorune::protocol_tag()))
            .into();

            // Get the WalletResponse for this address (full set of spendable outputs)
            match protorunes_by_address(env, &request.encode_to_vec()) {
                Ok(full_response) => {
                    // Cache the serialized full WalletResponse
                    CACHED_WALLET_RESPONSE
                        .select(&address)
                        .set(env, Arc::new(full_response.encode_to_vec()));

                    // Create a filtered version with only outpoints that have runes
                    let mut filtered_response = full_response.clone();
                    filtered_response.outpoints = filtered_response
                        .outpoints
                        .into_iter()
                        .filter_map(|v| {
                            if v.balances.unwrap_or_default().entries.len() == 0 {
                                None
                            } else {
                                Some(v)
                            }
                        })
                        .collect::<Vec<protorune_support::proto::protorune::OutpointResponse>>();

                    // Cache the serialized filtered WalletResponse
                    CACHED_FILTERED_WALLET_RESPONSE
                        .select(&address)
                        .set(env, Arc::new(filtered_response.encode_to_vec()));
                }
                Err(e) => {
                    env.log(&format!("Error caching wallet response for address: {:?}", e));
                }
            }
        }
    }

    logging::log_block_summary(env, block, height, block.total_size());
    Ok(())
}