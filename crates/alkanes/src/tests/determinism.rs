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
fn test_incoming_alkanes_ordered() -> Result<()> {
    clear();
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let self_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![22, 1000],
    };
    let copy_mint_cellpack = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![22, 1000],
    };

    let test_order_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![6],
    };
    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [self_mint_cellpack].into(),
    );

    for i in 1..10 {
        test_block.txdata.push(
            alkane_helpers::create_multiple_cellpack_with_witness_and_in(
                Witness::new(),
                vec![copy_mint_cellpack.clone()],
                OutPoint {
                    txid: test_block.txdata[i].compute_txid(),
                    vout: 0,
                },
                false,
            ),
        );
    }

    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![test_order_cellpack.clone()],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );

    index_block(&test_block, block_height)?;

    let outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_return_context(&outpoint, |trace_response| Ok(()))?;

    Ok(())
}
