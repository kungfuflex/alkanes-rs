use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use wasm_bindgen_test::wasm_bindgen_test;

fn read_alkane_binary(id: &AlkaneId) -> Vec<u8> {
    IndexPointer::from_keyword("/alkanes/")
        .select(&id.clone().into())
        .get()
        .as_ref()
        .clone()
}

/// Find the block-2 alkane id that holds exactly `target` (the deployed contract's
/// stored, compressed bytes), skipping genesis/reserved entries that occupy block 2.
fn id_holding(target: &[u8]) -> Option<AlkaneId> {
    for tx in 0u128..16 {
        let id = AlkaneId { block: 2, tx };
        if !target.is_empty() && read_alkane_binary(&id) == target {
            return Some(id);
        }
    }
    None
}

/// A contract deployed via the BIP-110-resistant hashlock envelope must be indexed and
/// stored byte-for-byte identically to one deployed via the legacy OP_FALSE OP_IF
/// envelope. Drives the in-memory (qubitcoin-backed) indexer end to end. Opcode 0 is the
/// test contract's `initialize` (a clean success); the deploy persists only if the
/// witness payload was extracted and decompressed correctly.
#[wasm_bindgen_test]
fn test_hashlock_deploy_parity() -> Result<()> {
    let block_height = 0;
    let wasm = alkanes_std_test_build::get_bytes();
    let compressed = compress(wasm.clone())?;
    let cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0],
    };

    // Legacy envelope deploy — locate the id holding the contract's compressed bytes.
    clear();
    let legacy_block = alkane_helpers::init_test_with_cellpack(cellpack.clone());
    index_block(&legacy_block, block_height)?;
    let deployed_id = id_holding(&compressed)
        .expect("legacy deploy did not persist the contract bytes in block 2");

    // Hashlock envelope deploy, fresh state — same bytes must land at the same id.
    clear();
    let hashlock_block = alkane_helpers::init_test_with_cellpack_hashlock(cellpack);
    index_block(&hashlock_block, block_height)?;

    assert_eq!(
        read_alkane_binary(&deployed_id),
        compressed,
        "hashlock deploy must store the contract's compressed bytes at {deployed_id:?}"
    );
    Ok(())
}
