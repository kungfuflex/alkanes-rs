mod helpers;
use alkanes_indexer::{indexer::index_block, message::AlkaneMessageContext};
use alkanes_support::{
    cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID, id::AlkaneId,
    utils::string_to_u128_list,
};
use crate::helpers::{
    self as alkane_helpers,
};
use anyhow::{anyhow, Result};
use bitcoin::{
    block::{Header as BlockHeader, Version},
    hashes::Hash,
    Block, CompactTarget, OutPoint,
};
use bitcoin::Witness;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::balance_sheet::BalanceSheetOperations;

#[tokio::test]
async fn test_owned_token() -> Result<()> {
    protorune_support::network::set_network(protorune_support::network::NetworkParams {
        bech32_prefix: String::from("bcrt"),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });
    let mut harness = helpers::TestHarness::new();
    harness.sync_config.exit_at = Some(0);
    let block_height = 0;

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0, 1, 1000],
    };
    let mint_test_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![1, 1000],
    };
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            vec![], // alkanes_std_auth_token_build::get_bytes(),
            vec![], // alkanes_std_owned_token_build::get_bytes(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test_cellpack, mint_test_cellpack].into(),
    );
    harness.add_block(test_block.clone());
    harness.process_block().await;
    let owned_token_id = AlkaneId { block: 2, tx: 1 };
    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 1,
    };
    let _sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    let _ = helpers::assert_binary_deployed_to_id(
        owned_token_id.clone(),
        vec![], // alkanes_std_owned_token_build::get_bytes(),
    );
    Ok(())
}