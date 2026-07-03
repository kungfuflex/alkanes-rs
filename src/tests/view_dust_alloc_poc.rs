//! PoC for the unbounded dust-output allocation in `simulate_protostones`
//! (src/view.rs synth-tx path).
//!
//! When the caller supplies no `transaction_bytes`, `simulate_protostones`
//! synthesizes a tx with `max_pointer + 1` dust outputs so every protostone
//! `pointer` has a real vout to land on (src/view.rs:957-968), then builds them
//! eagerly in `synth_tx_carrying_protostones` (src/view.rs:814). `pointer` is an
//! attacker-controlled `Option<u32>`, so a single protostone with
//! `pointer = u32::MAX` forces ~4.29 billion `TxOut`s — a capacity-overflow
//! panic / multi-GB OOM from one unauthenticated view RPC.
//!
//! Fix: clamp `num_dust_outputs` to a sane bound. Any pointer past the bound is
//! an invalid vout anyway and is rejected gracefully by `process_message`'s
//! existing "Invalid output pointer" check, so the simulate returns a normal
//! error response instead of crashing.
//!
//! Pre-fix `test_simulate_huge_pointer_no_oom` panics with "capacity overflow";
//! post-fix it returns without allocating billions of outputs.

use crate::tests::helpers::clear;
use crate::view::{simulate_protostones, SimulateProtostonesInput};
use anyhow::Result;
use protorune::protostone::Protostones;
use protorune_support::protostone::Protostone;
use protorune_support::utils::encode_varint_list;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_simulate_huge_pointer_no_oom() -> Result<()> {
    clear();

    // A single non-message protostone whose pointer is u32::MAX. On the synth
    // path this asks for ~4.29 billion dust outputs.
    let protostone = Protostone {
        message: vec![],
        protocol_tag: 1,
        from: None,
        burn: None,
        pointer: Some(u32::MAX),
        refund: Some(0),
        edicts: vec![],
    };
    let protostones_bytes = encode_varint_list(&vec![protostone].encipher()?);

    let result = simulate_protostones(SimulateProtostonesInput {
        height: 1,
        alkane_inputs: vec![],
        protostones_bytes,
        transaction_bytes: None, // synth-tx path -> allocates max_pointer+1 outputs
        block_bytes: None,
        storage_overrides: vec![],
    });

    // INVARIANT: a view request must never trigger an unbounded allocation.
    // Reaching this line at all proves no capacity-overflow/OOM occurred; the
    // clamp turns the over-large pointer into a normal (Ok, error-reporting)
    // response.
    assert!(
        result.is_ok(),
        "simulate must return gracefully for an out-of-range pointer, got {:?}",
        result.err()
    );
    Ok(())
}
