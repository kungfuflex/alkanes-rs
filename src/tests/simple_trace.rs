use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{self as alkane_helpers, assert_return_context};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use bitcoin::OutPoint;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_simple_trace() -> Result<(), anyhow::Error> {
    alkane_helpers::clear();
    let mut block = protorune::test_helpers::create_block_with_coinbase_tx(1);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        bitcoin::Witness::new(),
        vec![Cellpack {
            target: AlkaneId { block: 1, tx: 1 },
            inputs: vec![],
        }],
        OutPoint::default(),
        false,
    );
    block.txdata.push(tx.clone());
    crate::indexer::index_block(&block, 1)?;
    let outpoint = OutPoint {
        txid: tx.compute_txid(),
        vout: 1,
    };
    assert_return_context(&outpoint, |_| Ok(()))?;
    Ok(())
}
