//! Deliverable 2 — mainnet block 893514, the FIRST consensus divergence caused
//! by the dropped `2:0` height map, and a deterministic differential test of the
//! execution-path fix at that height.
//!
//! # The divergent transaction
//! Block 893514 (< `FUEL_CHANGE1_HEIGHT` = 899_087, so the tight pre-fork fuel
//! budget is in effect) creates the free-mint alkane `2:465` in tx
//! `0b3c9324d8df62ac59c318c9b1159f603c8ab5f2518f094a1e062cd9c3bc6024`. The
//! trimmed fixture `blocks/block_893514_mainnet.hex` = `[coinbase, 0b3c9324…]`
//! (the funding parent `b9e4b490…d803` is in an earlier block — BTC fee funding
//! only). `test_block_893514_divergent_tx_shape` decodes that tx and pins its
//! shape: a single alkanes protostone (protocol tag 1) whose cellpack targets
//! `6:835717` — a factory-create from template `4:835717` — which on execution
//! deploys `2:<sequence>` and, during construction, extcalls DIESEL `2:0`.
//!
//! That extcall is where the bug bit: with `2:0` resolved to the heavy
//! upgraded-EOA build (262_445 B) instead of the height-map base build
//! (174_225 B), construction over-consumed the pre-899_087 fuel budget and
//! reverted "all fuel consumed by WebAssembly", rolling back the create so the
//! sequence never advanced past 465 (whence the audit indexer's ~17x alkane
//! shortfall vs canonical mainnet).
//!
//! # Why the raw block is not replayed for a create assertion
//! Replaying `0b3c9324…` in isolation cannot reproduce the create: alkanes only
//! executes a protomessage that RECEIVES protorune balance (the real tx is fed
//! forwarded DIESEL from its prior-block free-mint-chain parent, absent here),
//! and the factory template `4:835717` is external mainnet state not carried in
//! the block bytes. So the on-the-nose "index the raw block -> 2:465" assertion
//! is not reconstructable from the fixture alone.
//!
//! # What IS proven deterministically
//! The bug is entirely in *which binary* `2:0` resolves to at a given height.
//! `test_get_alkane_binary_for_2_0_resolves_by_height_not_indexed_state` drives
//! the exact fixed function — `vm::utils::get_alkane_binary_from_context` — for
//! `2:0`, with the indexed `2:0` binary deliberately seeded to the WRONG build,
//! and asserts the returned code is chosen by the height map, not indexed state:
//!   * height 893514 (the divergence height) -> the light base build (174_225 B)
//!     even though indexed state holds the heavy upgraded-EOA build;
//!   * height 917888 -> the upgraded-EOA build (262_445 B) even though indexed
//!     state holds the light base build.
//! Both directions FAIL on the pre-fix code (which dropped the `2:0` case and
//! read indexed state): at 893514 it would return the 262_445 B EOA build (the
//! fuel-exhausting binary), at 917888 the 174_225 B base build.

#![cfg(feature = "mainnet")]

use crate::network::{
    genesis_alkane_bytes, genesis_alkane_upgrade_bytes_eoa, genesis_alkane_wasm_for_height,
};
use crate::tests::helpers as alkane_helpers;
use crate::vm::runtime::AlkanesRuntimeContext;
use crate::vm::utils::get_alkane_binary_from_context;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use bitcoin::Block;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::message::MessageContextParcel;
use protorune_support::utils::consensus_decode;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use wasm_bindgen_test::wasm_bindgen_test;

#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};

const BLOCK_893514_HEX: &str = include_str!("blocks/block_893514_mainnet.hex");
const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };
const CHILD_TXID: &str = "0b3c9324d8df62ac59c318c9b1159f603c8ab5f2518f094a1e062cd9c3bc6024";
const BASE_LEN: usize = 174_225;
const EOA_LEN: usize = 262_445;

fn load_trimmed_block() -> Block {
    let block_bytes = hex::decode(BLOCK_893514_HEX.trim()).expect("decode block hex");
    consensus_decode::<Block>(&mut Cursor::new(block_bytes)).expect("parse block 893514")
}

/// Overwrite the indexed `2:0` binary with a given build (compressed, matching
/// how the indexer stores program bytes) — simulating the wrong seed the fix
/// must override.
fn seed_indexed_diesel(bytes: Vec<u8>) {
    let compressed = compress(bytes).expect("compress genesis binary");
    IndexPointer::from_keyword("/alkanes/")
        .select(&DIESEL.into())
        .set(Arc::new(compressed));
}

/// Resolve the code `2:0` would execute at `height`, going through the exact
/// fixed dispatch (`get_alkane_binary_from_context`). `message.atomic` defaults
/// to the global store where `seed_indexed_diesel` wrote, so the pre-fix
/// fallthrough would read that seed.
fn resolve_diesel_binary_at(height: u64) -> Vec<u8> {
    let mut parcel = MessageContextParcel::default();
    parcel.height = height;
    let ctx = AlkanesRuntimeContext::from_parcel_and_cellpack(&parcel, &Cellpack::default());
    let arc = Arc::new(Mutex::new(ctx));
    get_alkane_binary_from_context(arc, &DIESEL)
        .expect("resolve 2:0 binary")
        .as_ref()
        .clone()
}

/// Pin the shape of the real divergent tx `0b3c9324…`: a single alkanes
/// protostone whose cellpack targets `6:835717` (a factory-create from template
/// `4:835717`) — the create whose construction extcalls DIESEL `2:0`.
#[wasm_bindgen_test]
fn test_block_893514_divergent_tx_shape() {
    use ordinals::{Artifact, Runestone};
    use protorune_support::protostone::Protostone;

    let block = load_trimmed_block();
    assert_eq!(block.txdata.len(), 2, "trimmed fixture = [coinbase, child]");
    let child = &block.txdata[1];
    assert_eq!(
        child.compute_txid().to_string(),
        CHILD_TXID,
        "second tx must be the 2:465 creating tx"
    );

    let rs = match Runestone::decipher(child) {
        Some(Artifact::Runestone(rs)) => rs,
        _ => panic!("child tx must carry a runestone"),
    };
    let protostones = Protostone::from_runestone(&rs).expect("decode protostones");
    assert_eq!(protostones.len(), 1, "one protostone on the divergent tx");
    let ps = &protostones[0];
    assert_eq!(ps.protocol_tag, 1, "alkanes protocol tag");
    assert!(!ps.message.is_empty(), "protomessage carries a cellpack");

    let varints =
        protorune_support::utils::decode_varint_list(&mut Cursor::new(ps.message.clone()))
            .expect("decode varint list");
    let cellpack = Cellpack::try_from(varints).expect("decode cellpack");
    println!(
        "divergent cellpack target = {}:{}",
        cellpack.target.block, cellpack.target.tx
    );
    assert_eq!(cellpack.target.block, 6, "factory-create opcode space");
    assert_eq!(cellpack.target.tx, 835_717, "factory template index");
    assert_eq!(
        cellpack.target.factory(),
        Some(AlkaneId {
            block: 4,
            tx: 835_717
        }),
        "6:835717 -> factory template 4:835717 (construction extcalls DIESEL 2:0)"
    );
}

/// The consensus-critical proof: `2:0`'s executable code is chosen by block
/// height (via the restored map), NOT by indexed state — so the seed can never
/// make `2:0` run the wrong (fuel-exhausting) binary at a given height.
///
/// Case A locks in the exact divergence: at height 893514 the light base build
/// runs even though indexed state holds the heavy EOA build. Case B proves the
/// map is a real progression (not a constant): at 917888 the EOA build runs even
/// though indexed state holds base. Both FAIL on the pre-fix code, which read
/// indexed state for `2:0`.
#[wasm_bindgen_test]
fn test_get_alkane_binary_for_2_0_resolves_by_height_not_indexed_state() {
    // Anchor to the real build sizes.
    assert_eq!(genesis_alkane_bytes().len(), BASE_LEN);
    assert_eq!(genesis_alkane_upgrade_bytes_eoa().len(), EOA_LEN);
    // The map itself agrees (guards against a mismatch between map + dispatch).
    assert_eq!(genesis_alkane_wasm_for_height(893_514).len(), BASE_LEN);
    assert_eq!(genesis_alkane_wasm_for_height(917_888).len(), EOA_LEN);

    // --- Case A: divergence height, WRONG seed = heavy EOA build ---------------
    alkane_helpers::clear();
    seed_indexed_diesel(genesis_alkane_upgrade_bytes_eoa());
    // Sanity: the seed really is the EOA build in indexed state.
    assert!(
        IndexPointer::from_keyword("/alkanes/")
            .select(&DIESEL.into())
            .get()
            .len()
            > 0,
        "indexed 2:0 seeded"
    );
    let at_893514 = resolve_diesel_binary_at(893_514);
    assert_eq!(
        at_893514.len(),
        BASE_LEN,
        "at h=893514, 2:0 must resolve to the base build ({} B) via the height \
         map, NOT the EOA-seeded indexed state ({} B) — got {} B",
        BASE_LEN,
        EOA_LEN,
        at_893514.len()
    );
    assert_eq!(
        at_893514,
        genesis_alkane_bytes(),
        "resolved bytes must be byte-identical to the base build"
    );

    // --- Case B: post-EOA height, WRONG seed = light base build ----------------
    alkane_helpers::clear();
    seed_indexed_diesel(genesis_alkane_bytes());
    let at_917888 = resolve_diesel_binary_at(917_888);
    assert_eq!(
        at_917888.len(),
        EOA_LEN,
        "at h=917888, 2:0 must resolve to the EOA build ({} B) via the height \
         map, NOT the base-seeded indexed state ({} B) — got {} B",
        EOA_LEN,
        BASE_LEN,
        at_917888.len()
    );
    assert_eq!(
        at_917888,
        genesis_alkane_upgrade_bytes_eoa(),
        "resolved bytes must be byte-identical to the EOA build"
    );

    println!("✓ 2:0 code resolves by height (base@893514, EOA@917888), ignoring indexed seed");
}
