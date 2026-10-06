//! Regression test for the factory self-alias stack-exhaustion DoS in
//! `get_alkane_binary` (src/vm/utils.rs).
//!
//! A public protocol-tag-1 Cellpack targeting factory `5:N`, where `N` is the
//! current public sequence, allocates `2:N` and stores the alias `2:N -> 2:N`.
//! Alias resolution used to recurse with no cycle or depth check, exhausting
//! the native/wasm call stack before the message could return an error — a
//! single unprivileged tx could halt block indexing.
//!
//! The fix resolves alias chains iteratively with cycle detection and a depth
//! cap, so the self-alias becomes an ordinary message failure that the block's
//! atomic checkpoint rolls back. `index_block` must therefore complete; the
//! staged sequence increment and alias write must be reverted — exactly the
//! behavior of the missing-future-factory control.

use crate::indexer::index_block;
use crate::tests::helpers::{
    assert_id_points_to_alkane_id, assert_token_id_has_no_deployment,
    create_multiple_cellpack_with_witness_and_in,
};
use crate::{network, tests::helpers, view};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use bitcoin::opcodes::all::OP_PUSHNUM_1;
use bitcoin::{Block, OutPoint, ScriptBuf, Witness};
use std::convert::TryInto;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const MATURITY: u32 = 101;

struct PublicSetup {
    current_sequence: u128,
    funding_outpoint: OutPoint,
    witness_script: ScriptBuf,
    attack_height: u32,
}

fn read_sequence() -> Result<u128> {
    let bytes = view::sequence()?;
    let fixed: [u8; 16] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("sequence view returned {} bytes", bytes.len()))?;
    Ok(u128::from_le_bytes(fixed))
}

fn bootstrap_public_state() -> Result<PublicSetup> {
    helpers::clear();

    // P2WSH(OP_TRUE) is a consensus-valid, signatureless funding output. The
    // attack block is placed 101 heights later to satisfy coinbase maturity.
    let witness_script = ScriptBuf::builder().push_opcode(OP_PUSHNUM_1).into_script();
    let mut funding_block = create_block_with_coinbase_tx(network::genesis::GENESIS_BLOCK as u32);
    funding_block.txdata[0].output[0].script_pubkey = witness_script.to_p2wsh();
    funding_block.header.merkle_root = funding_block
        .compute_merkle_root()
        .ok_or_else(|| anyhow!("funding block has no merkle root"))?;
    let funding_outpoint = OutPoint {
        txid: funding_block.txdata[0].compute_txid(),
        vout: 0,
    };

    index_block(&funding_block, network::genesis::GENESIS_BLOCK as u32)?;
    let current_sequence = read_sequence()?;
    if current_sequence == 0 {
        return Err(anyhow!("genesis did not initialize the public sequence"));
    }

    Ok(PublicSetup {
        current_sequence,
        funding_outpoint,
        witness_script,
        attack_height: network::genesis::GENESIS_BLOCK as u32 + MATURITY,
    })
}

use protorune::test_helpers::create_block_with_coinbase_tx;

fn public_factory_block(setup: &PublicSetup, factory_tx: u128) -> Block {
    let mut witness = Witness::new();
    witness.push(setup.witness_script.as_bytes());
    let tx = create_multiple_cellpack_with_witness_and_in(
        witness,
        vec![Cellpack {
            target: AlkaneId {
                block: 5,
                tx: factory_tx,
            },
            // Opcode 99 is GetName for the 2:0 precompiled genesis alkane.
            // Resolution occurs before dispatch, so the attack does not depend
            // on this opcode or on contract-specific state.
            inputs: vec![99],
        }],
        setup.funding_outpoint,
        false,
    );
    let mut block = create_block_with_coinbase_tx(setup.attack_height);
    block.txdata.push(tx);
    block.header.merkle_root = block.compute_merkle_root().expect("attack merkle root");
    block
}

/// Control: a factory targeting the *previous* sequence resolves a real binary
/// and deploys normally.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn control_previous_factory_target_succeeds() -> Result<()> {
    let setup = bootstrap_public_state()?;
    let previous = setup
        .current_sequence
        .checked_sub(1)
        .ok_or_else(|| anyhow!("sequence has no prior factory"))?;
    let block = public_factory_block(&setup, previous);

    index_block(&block, setup.attack_height)?;

    assert_eq!(read_sequence()?, setup.current_sequence + 1);
    assert_id_points_to_alkane_id(
        AlkaneId {
            block: 2,
            tx: setup.current_sequence,
        },
        AlkaneId {
            block: 2,
            tx: previous,
        },
    )?;
    Ok(())
}

/// Control: a factory targeting a *future* (not-yet-allocated) sequence is a
/// normal message failure; the block still indexes and the staged writes roll
/// back.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn control_future_factory_target_fails_bounded_and_rolls_back() -> Result<()> {
    let setup = bootstrap_public_state()?;
    let future = setup.current_sequence + 1;
    let block = public_factory_block(&setup, future);

    index_block(&block, setup.attack_height)?;

    assert_eq!(read_sequence()?, setup.current_sequence);
    assert_token_id_has_no_deployment(AlkaneId {
        block: 2,
        tx: setup.current_sequence,
    })?;
    Ok(())
}

/// The regression: a factory targeting the *current* sequence aliases the
/// Alkane ID being allocated (`2:N -> 2:N`). Pre-fix this exhausted the
/// resolver stack and aborted indexing. Post-fix it is a bounded message
/// failure: `index_block` returns `Ok`, and the sequence and alias roll back
/// exactly like the future-factory control.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn attack_current_sequence_factory_self_alias() -> Result<()> {
    let setup = bootstrap_public_state()?;
    let block = public_factory_block(&setup, setup.current_sequence);

    let result = index_block(&block, setup.attack_height);

    assert!(
        result.is_ok(),
        "INVARIANT: a self-aliasing factory call must be a bounded message \
         revert, never a stack-exhausting panic that halts index_block; got {:?}",
        result.err()
    );
    assert_eq!(
        read_sequence()?,
        setup.current_sequence,
        "the staged sequence increment must roll back on failure"
    );
    assert_token_id_has_no_deployment(AlkaneId {
        block: 2,
        tx: setup.current_sequence,
    })?;
    Ok(())
}
