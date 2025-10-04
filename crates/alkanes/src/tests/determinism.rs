use crate::tests::std::alkanes_std_test_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};

use crate::indexer::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::test_runtime::TestRuntime;

#[test]
fn test_incoming_alkanes_ordered() -> Result<()> {

    let mut env = TestRuntime::default();
    alkane_helpers::clear(&mut env);
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

    index_block::<TestRuntime>(&mut env, &test_block, block_height)?;

    let outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 3,
    };

    alkane_helpers::assert_return_context(&mut env, &outpoint, |trace_response| Ok(()))?;

    Ok(())
}