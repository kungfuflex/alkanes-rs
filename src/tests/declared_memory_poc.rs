//! Regression test for the attacker-controlled memory-maximum panic in
//! `AlkanesInstance::from_alkane` (src/vm/instance.rs).
//!
//! A public CREATE deployment carries its module bytes in the witness. After
//! Wasmi instantiates that attacker-controlled module, `from_alkane` forces the
//! guest memory up to 512 pages. A module declaring a memory *maximum* below
//! 512 pages makes that growth fail with `OutOfBoundsGrowth`. Before the fix
//! the result was `.expect("Failed to grow memory")`, so the unhandled panic
//! aborted `index_block` — a single cheap, unprivileged tx could halt indexing.
//!
//! The fix propagates the growth failure as an `anyhow::Error`, which the
//! message checkpoint turns into an ordinary per-message revert. `index_block`
//! must therefore complete for every case below, including the sub-512 one;
//! only the deployment's own execution reverts.

use crate::index_block;
use crate::tests::helpers::clear;
use alkanes_support::{cellpack::Cellpack, envelope::RawEnvelope, id::AlkaneId};
use anyhow::Result;
use protorune::test_helpers::create_block_with_coinbase_tx;
use wasm_bindgen_test::wasm_bindgen_test;

const HEIGHT: u32 = 880_001;

/// A minimal, valid alkane whose only interesting property is its declared
/// memory section. `__execute` returns a pointer to a zeroed
/// `ExtendedCallResponse` (0 transfers, 0 storage pairs) at offset 4; offset 0
/// holds the 20-byte little-endian ArrayBuffer length prefix.
fn module(memory_decl: &str) -> Vec<u8> {
    let response = "\\14\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00\\00";
    let src = format!(
        "(module\n  (memory (export \"memory\") {memory_decl})\n  (data (i32.const 0) \"{response}\")\n  (func (export \"__execute\") (result i32)\n    i32.const 4)\n)\n",
    );
    wat::parse_str(src).expect("regression WAT must compile")
}

fn deploy_block(memory_decl: &str) -> bitcoin::Block {
    let mut block = create_block_with_coinbase_tx(HEIGHT);
    let witness = RawEnvelope::from(module(memory_decl)).to_witness(true);
    block.txdata.push(crate::tests::helpers::create_cellpack_with_witness(
        witness,
        Cellpack {
            target: AlkaneId::new(1, 0),
            inputs: vec![],
        },
    ));
    block
}

/// The regression: a module declaring `(memory 1 1)` — maximum 1 page, far
/// below the required 512 — must NOT panic the indexer. `index_block` returns
/// `Ok`; the deployment simply reverts.
#[wasm_bindgen_test]
fn test_declared_memory_below_512_does_not_halt_indexer() -> Result<()> {
    clear();
    let result = index_block(&deploy_block("1 1"), HEIGHT);
    assert!(
        result.is_ok(),
        "INVARIANT: a sub-512-page memory maximum must revert the message, \
         never panic index_block; got {:?}",
        result.err()
    );
    Ok(())
}

/// Controls: a maximum of exactly 512, no maximum, and a maximum above 512 all
/// grow cleanly and index without error.
#[wasm_bindgen_test]
fn test_declared_memory_valid_maxima_still_index() -> Result<()> {
    for decl in ["1 512", "1", "1 513"] {
        clear();
        let result = index_block(&deploy_block(decl), HEIGHT);
        assert!(
            result.is_ok(),
            "control deployment with memory decl '{decl}' must index cleanly; got {:?}",
            result.err()
        );
    }
    Ok(())
}
