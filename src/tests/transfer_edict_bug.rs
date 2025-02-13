use crate::{message::AlkaneMessageContext, tests::std::alkanes_std_auth_token_build};
use alkanes_support::id::AlkaneId;
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use protorune_support::balance_sheet::ProtoruneRuneId;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::alkanes_std_owned_token_build;
use crate::tests::forge::create_protostone_encoded_transaction;
use alkane_helpers::clear;
use protorune::test_helpers as helpers;

#[allow(unused_imports)]
use metashrew::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_transfer_edict_duplication() -> Result<()> {
    println!("\n=== Starting Transfer Edict Duplication Test ===");
    clear();
    let block_height = 840_000;

    println!("\n[1] Initializing tokens...");
    // Initialize an owned token and auth token
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    println!("Created auth token cellpack with target {:?}", auth_cellpack.target);

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    println!("Created test cellpack with target {:?} and inputs {:?}", test_cellpack.target, test_cellpack.inputs);

    println!("\n[2] Creating initial block...");
    // Create and index initial block with tokens
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test_cellpack].into(),
    );
    println!("Created block with {} transactions", test_block.txdata.len());

    println!("\n[3] Indexing initial block at height {}...", block_height);
    index_block(&test_block, block_height)?;

    // Get the outpoint containing the initialized tokens
    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    println!("Initial tokens outpoint: {:?}", outpoint);
    println!("Loading initial balance sheet...");
    let owned_token_id = AlkaneId { block: 2, tx: 1 };
    let auth_token_id = AlkaneId { block: 2, tx: 2 };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    println!("Balance sheet loaded successfully");

    // Verify initial balances
    let owned_balance = sheet.get(&owned_token_id.into());
    let auth_balance = sheet.get(&auth_token_id.into());
    println!(
        "Initial balances - owned: {}, auth: {}",
        owned_balance, auth_balance
    );

    println!("\n[4] Creating protostone with transfer edict (will revert)...");
    // Create a protostone with transfer edict that will revert
    let protostone = Protostone {
        protocol_tag: AlkaneMessageContext::protocol_tag(),
        from: None,
        edicts: vec![ProtostoneEdict {
            id: ProtoruneRuneId {
                block: 2,  // Owned token alkane [2,0] 
                tx: 1
            },
            amount: 100,
            output: 0
        }],
        pointer: Some(0),
        refund: Some(0),
        message: vec![2, 0, 1], // calldata with opcode [1] that will revert
        burn: None
    };
    println!("Created protostone with protocol_tag {}, targeting alkane [2,1], attempting to transfer 100 units and calling the 0 opcode on genesis alkane [2,0] which will revert", protostone.protocol_tag);

    println!("\n[5] Creating new block for transfer transaction...");
    // Create a new block with just the transfer transaction
    let transfer_tx = create_protostone_encoded_transaction(
        outpoint, 
        vec![protostone]
    );
    let mut transfer_block = helpers::create_block_with_coinbase_tx(block_height);
    transfer_block.txdata.push(transfer_tx);
    println!("Created transfer block with {} transactions", transfer_block.txdata.len());

    println!("\n[6] Indexing block with reverting transfer...");
    // Index the block with the protostone
    index_block(&transfer_block, block_height)?;

    println!("\n[7] Checking final state...");
    // Check final state - verify no duplication occurred
    let edict_outpoint = OutPoint {
        txid: transfer_block.txdata[1].compute_txid(), // index 1 since 0 is coinbase
        vout: 0
    };
    println!("Checking balance sheet at outpoint: {:?}", edict_outpoint);
    
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&edict_outpoint)?),
    );
    let new_owned_balance = sheet.get(&owned_token_id.into());
    let new_auth_balance = sheet.get(&auth_token_id.into());
    println!(
        "New balances - owned: {}, auth: {}",
        new_owned_balance, new_auth_balance
    );

    
    println!("\n[8] Verifying no duplication occurred...");
    // Verify balance hasn't been duplicated (should still be 1000 or less)
    assert!(new_owned_balance <= owned_balance, 
        "Owned token new balance {} is greater than original balance {} - duplication detected!", 
        new_owned_balance, owned_balance
    );
    
    println!("✓ Test passed: Owned token balance ({}) not duplicated", new_owned_balance);
    
    
    
    Ok(())
}