use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::{
    alkanes_std_auth_token_build, alkanes_std_test_build, alkanes_std_upgradeable_build,
};
use alkane_helpers::clear;
use alkanes::view;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::{Block, Transaction};
use bitcoin::{OutPoint, Witness};
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::utils::consensus_decode;
use wasm_bindgen_test::wasm_bindgen_test;

fn upgradeability_harness() -> Result<Block> {
    clear();
    let block_height = 840_000;
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };
    let upgrade = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0x7fff, 2, 1, 1],
    };
    let set_claimable = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![104, 10],
    };
    let mint = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![22, 1_000_000],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
            alkanes_std_upgradeable_build::get_bytes(),
            [].into(),
            [].into(),
        ]
        .into(),
        [auth_cellpack, test, upgrade, set_claimable, mint].into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 2 }),
        1_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    Ok(test_block)
}

#[wasm_bindgen_test]
fn test_proxy() -> Result<()> {
    upgradeability_harness()?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_upgradeability() -> Result<()> {
    let init_block = upgradeability_harness()?;
    let block_height = 840_001;
    let test = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![50],
    };

    let deploy_new =
        alkane_helpers::init_with_multiple_cellpacks_with_tx([[].into()].into(), [test].into());

    index_block(&deploy_new, block_height)?;

    let upgrade = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![0x7ffe, 2, 4],
    };
    let get_claimable = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![103],
    };
    let mint = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![22, 1_000_000],
    };

    let mut test_block = create_block_with_coinbase_tx(block_height + 1);

    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![upgrade, get_claimable, mint],
            OutPoint {
                txid: init_block.txdata[init_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );

    index_block(&test_block, block_height + 1)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 2 }),
        2_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 4,
    };

    let raw_trace_data = view::trace(&outpoint)?;
    let trace_data: Trace = raw_trace_data.clone().try_into()?;

    let trace_event_1 = trace_data.0.lock().expect("Mutex poisoned").last().cloned();

    // Access the data field from the trace response
    if let Some(return_context) = trace_event_1 {
        // Use pattern matching to extract the data field from the TraceEvent enum
        match return_context {
            TraceEvent::ReturnContext(trace_response) => {
                // Now we have the TraceResponse, access the data field
                let data = trace_response.inner.data;

                assert_eq!(data[0], 10);
            }
            _ => panic!("Expected ReturnContext variant, but got a different variant"),
        }
    } else {
        panic!("Failed to get trace_event_1 from trace data");
    }

    Ok(())
}
