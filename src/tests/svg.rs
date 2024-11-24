// #![cfg(test)]

// use bitcoin::{Address, Amount, Network, ScriptBuf, Sequence, TxIn, TxOut, Witness};
// use bitcoin::blockdata::transaction::OutPoint;
// use bitcoin::transaction::Version;
// use protorune_support::protostone::Protostone;
// use std::str::FromStr;
// use anyhow::Result;
// use bitcoin_hashes::Hash;
// use alkanes_runtime::runtime::AlkaneResponder;
// use alkanes_std_svg::NounsArt;
// use alkanes_support::context::Context;
// use metashrew_support::index_pointer::KeyValuePointer;
// use crate::tests::std::alkanes_std_test_build;
// use alkanes_support::cellpack::Cellpack;
// use alkanes_support::id::AlkaneId;
// use wasm_bindgen_test::wasm_bindgen_test;
// use crate::index_block;
// use crate::tests::helpers as alkane_helpers;
// use alkanes::message::AlkaneMessageContext;
// use protorune::tables::RuneTable;
// use protorune_support::utils::consensus_encode;
// use metashrew::clear;

// //
// // Test helper function 
// fn create_test_transaction(
//     inputs: Vec<TxIn>,
//     outputs: Vec<TxOut>,
//     _protostone: Protostone
// ) -> bitcoin::Transaction {
//     bitcoin::Transaction {
//         version: Version(2),
//         lock_time: bitcoin::absolute::LockTime::ZERO,
//         input: inputs,
//         output: outputs,
//     }
// }

// #[wasm_bindgen_test]
// fn test_svg_initialize() -> Result<()> {
//     clear();
//     let block_height = 840_000;

//     let test_cellpack = Cellpack {
//         target: AlkaneId { block: 1, tx: 0 },
//         inputs: vec![10],
//     };

//     let test_block = alkane_helpers::init_test_with_cellpack(test_cellpack);
//     index_block(&test_block, block_height)?;

//     let outpoint = bitcoin::OutPoint {
//         txid: test_block.txdata.last().unwrap().compute_txid(),
//         vout: 0,
//     };


    
//     Ok(())
// }

// #[wasm_bindgen_test]
// fn test_svg_trait_operations() -> Result<()> {
//     clear();
//     let block_height = 840_000;

//     let trait_tests = vec![
//         (10, "/bodies"),
//         (11, "/accessories"),
//         (12, "/heads"),
//         (13, "/glasses"),
//         (14, "/palettes"),
//     ];

//     for (op_code, _trait_path) in trait_tests {
//         let test_cellpack = Cellpack {
//             target: AlkaneId { block: 1, tx: 0 },
//             inputs: vec![op_code],
//         };

//         let test_block = alkane_helpers::init_test_with_cellpack(test_cellpack);
//         index_block(&test_block, block_height)?;

//         let outpoint = bitcoin::OutPoint {
//             txid: test_block.txdata.last().unwrap().compute_txid(),
//             vout: 0,
//         };


//     }
//     Ok(())
// } 