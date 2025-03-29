use crate::{message::AlkaneMessageContext, tests::std::alkanes_std_auth_token_build};
use alkanes_support::id::AlkaneId;
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_binary_deployed_to_id};
use crate::tests::std::alkanes_std_owned_token_build;
use alkane_helpers::clear;
#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_owned_token_mint_crash() -> Result<()> {
    clear();
    let block_height = 840_000;

    // First deploy auth token factory
    let auth_factory_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    // Deploy and initialize owned token
    let owned_token_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    // opcode (initialize)
            1,    // auth_token units
            1000, // initial token supply
        ],
    };

    // Create mint operation cellpack that causes crash
    let mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 }, // Points to the owned token
        inputs: vec![
            77,  // mint opcode
            500, // amount to mint
        ],
    };

    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_factory_cellpack, owned_token_cellpack].into(),
    );

    println!("STEP 1: Indexing initial deployment block...");
    index_block(&test_block, block_height)?;
    println!("STEP 1: Initial deployment block indexed successfully");

    let owned_token_id = AlkaneId { block: 2, tx: 1 };
    let auth_token_id = AlkaneId { block: 2, tx: 2 };
    println!(
        "STEP 2: Created token IDs: owned={:?}, auth={:?}",
        owned_token_id, auth_token_id
    );

    // Verify initial state
    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    println!("STEP 3: Got outpoint: {:?}", outpoint);

    println!("STEP 4: Loading initial balance sheet...");
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    println!("STEP 4: Balance sheet loaded successfully");

    // Verify initial balances
    let owned_balance = sheet.get(&owned_token_id.into());
    let auth_balance = sheet.get(&auth_token_id.into());
    println!(
        "STEP 5: Initial balances - owned: {}, auth: {}",
        owned_balance, auth_balance
    );
    assert_eq!(owned_balance, 1000, "Initial token balance incorrect");
    assert_eq!(auth_balance, 1, "Auth token balance incorrect");

    println!("STEP 6: Creating mint block...");
    let mint_block = alkane_helpers::init_with_multiple_cellpacks(
        alkanes_std_owned_token_build::get_bytes(),
        vec![mint_cellpack.clone()],
    );
    println!("STEP 6: Mint block created successfully");

    println!("STEP 7: About to index mint block...");

    index_block(&mint_block, block_height + 1)?;
    println!("STEP 8: Mint block indexed successfully");

    // Get the mint transaction info
    println!("STEP 9: Checking mint transaction state...");
    let mint_tx = mint_block.txdata.last().ok_or(anyhow!("no mint tx"))?;
    let mint_outpoint = OutPoint {
        txid: mint_tx.compute_txid(),
        vout: 0,
    };
    let mint_sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&mint_outpoint)?),
    );
    println!(
        "STEP 10: Mint state - txid: {}, balances: {:?}",
        mint_tx.compute_txid(),
        mint_sheet.balances
    );

    println!("Test completed successfully - no crash occurred");

    Ok(())
}
