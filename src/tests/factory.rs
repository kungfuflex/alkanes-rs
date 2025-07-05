use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::ProtostoneEdict;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, get_sheet_for_runtime};
use alkane_helpers::clear;
use alkanes::view;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use protorune_support::balance_sheet::ProtoruneRuneId;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_factory_wasm_load() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 1_000_000],
    };

    let send_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![3],
    };

    let create_another_cellpack = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![50],
    };

    let steal_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![30, 2, 2, 1_000_000],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_test_build::get_bytes(),
            [].into(),
            [].into(),
            [].into(),
        ]
        .into(),
        [
            arb_mint_cellpack,
            send_cellpack,
            create_another_cellpack,
            steal_cellpack,
        ]
        .into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);
    let runtime_sheet = get_sheet_for_runtime();

    assert_eq!(
        runtime_sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }),
        1000000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 2 }),
        1000000
    );
    Ok(())
}
