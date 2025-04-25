use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::ProtostoneEdict;

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
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
fn test_transfer_overflow() -> Result<()> {
    clear();
    println!(
        "USER SHOULD EXPECT ERROR IN LOGS: 'err: overflow error during balance sheet increase'"
    );
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, u128::MAX],
    };
    let arb_mint_cellpack2 = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![30, 2, 1, u128::MAX],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes(), [].into()].into(),
        [arb_mint_cellpack, arb_mint_cellpack2.clone()].into(),
    );

    index_block(&test_block, block_height)?;

    let mut test_block2 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [[].into()].into(),
        [arb_mint_cellpack2].into(),
    );

    let input_script = ScriptBuf::new();
    let txin1 = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: input_script.clone(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };
    let txin2 = TxIn {
        previous_output: OutPoint {
            txid: test_block2.txdata[test_block2.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: input_script.clone(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    test_block2.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_txins_edicts(
            vec![Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![50],
            }],
            vec![txin1, txin2],
            false,
            vec![],
        ),
    );

    index_block(&test_block2, block_height + 1)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    Ok(())
}

#[wasm_bindgen_test]
fn test_mint_overflow() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, u128::MAX],
    };
    let arb_mint_cellpack2 = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![30, 2, 1, u128::MAX],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );

    let input_script = ScriptBuf::new();
    let txin1 = TxIn {
        previous_output: OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        script_sig: input_script.clone(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_txins_edicts(
            vec![arb_mint_cellpack2.clone()],
            vec![txin1],
            false,
            vec![],
        ),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }),
        340282366920938463463374607431768211455
    ); // it refunded

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(&outpoint, "overflow error during balance sheet increase, current(340282366920938463463374607431768211455) + additional(340282366920938463463374607431768211455)")?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_mint_underflow() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 0, 1_000_000],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [arb_mint_cellpack].into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(&outpoint, "overflow error")?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_extcall_mint() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![31, 2, 1, 3, 30, 2, 0, 1_000_000],
    };

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![arb_mint_cellpack],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(
        &outpoint,
        "ALKANES: revert: Error: Extcall failed: balance underflow during transfer_from",
    )?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_delegatecall_mint() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![32, 2, 1, 3, 30, 2, 0, 1_000_000],
    };

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![arb_mint_cellpack],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(
        &outpoint,
        "ALKANES: revert: Error: Extcall failed: balance underflow during transfer_from",
    )?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_extcall_mint_err_plus_good_protostone() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![31, 2, 1, 3, 30, 2, 0, 1_000_000],
    };
    let mint_self_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![30, 2, 1, 1_000_000],
    };

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![arb_mint_cellpack, mint_self_cellpack],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(
        &outpoint,
        "ALKANES: revert: Error: Extcall failed: balance underflow during transfer_from",
    )?;

    alkane_helpers::assert_revert_context(
        &OutPoint {
            txid: test_block.txdata.last().unwrap().compute_txid(),
            vout: 4,
        },
        "ALKANES: revert: all fuel consumed by WebAssembly",
    )?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_multiple_extcall_err_and_good() -> Result<()> {
    clear();
    let block_height = 840_000;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_test_build::get_bytes()].into(),
        [init_cellpack].into(),
    );

    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![
            34, 2, 1, 3, 30, 2, 0, 1_000_000, 2, 1, 3, 30, 2, 1, 1_000_000,
        ],
    };

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![arb_mint_cellpack],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;

    println!("Last sheet: {:?}", sheet);

    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 0 }), 0);
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata.last().unwrap().compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_revert_context(&outpoint, "ALKANES: revert")?;

    Ok(())
}
