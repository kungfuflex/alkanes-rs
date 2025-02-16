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
fn test_owned_token() -> Result<()> {
    clear();
    let block_height = 840_000;

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
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test_cellpack, mint_test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;
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
    /*
        let _ = assert_binary_deployed_to_id(
            owned_token_id.clone(),
            alkanes_std_owned_token_build::get_bytes(),
        );
    */
    Ok(())
}
#[wasm_bindgen_test]
fn test_auth_and_owned_token_noop() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![100],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let _sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    // assert_eq!(sheet.get(&original_rune_id.into()), 1000);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_auth_and_owned_token() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let auth_token_id_deployment = AlkaneId { block: 2, tx: 2 };
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        auth_token_id_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}

#[wasm_bindgen_test]
fn test_auth_and_owned_token_multiple() -> Result<()> {
    clear();
    let block_height = 840_000;

    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };

    let test_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let owned_copy_cellpack = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![
            0,    /* opcode (to init new auth token) */
            1,    /* auth_token units */
            1000, /* owned_token token_units */
        ],
    };
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test_cellpack, owned_copy_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let _auth_token_id_factory = AlkaneId {
        block: 4,
        tx: AUTH_TOKEN_FACTORY_ID,
    };

    let auth_token_id_deployment = AlkaneId { block: 2, tx: 2 };
    let owned_token_id = AlkaneId { block: 2, tx: 1 };

    let tx = test_block.txdata.last().ok_or(anyhow!("no last el"))?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&owned_token_id.into()), 1000);
    assert_eq!(sheet.get(&auth_token_id_deployment.into()), 1);

    let tx_first = test_block.txdata.first().ok_or(anyhow!("no first el"))?;
    let outpoint_first = OutPoint {
        txid: tx_first.compute_txid(),
        vout: 0,
    };
    let sheet_first = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint_first)?),
    );
    assert_eq!(sheet_first.balances.len(), 0);
    let _ = assert_binary_deployed_to_id(
        owned_token_id.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        _auth_token_id_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        auth_token_id_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    Ok(())
}
