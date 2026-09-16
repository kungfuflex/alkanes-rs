//! v3 `address-indexing` opt-in feature.
//!
//! ## Status
//!
//! OFF by default. Gated behind the `address-indexing` Cargo feature.
//! The canonical mainnet wasm build does NOT enable this feature —
//! address-keyed lookups are served from esplora's UTXO API via espo
//! middleware. Operators who need an indexer-served address-by-x
//! surface opt in with:
//!
//! ```text
//! cargo build --release --target wasm32-unknown-unknown \
//!   --features mainnet,address-indexing -p alkanes
//! ```
//!
//! ## Storage shape
//!
//! ```text
//! /v3/addr/<address-bytes>  ->  chunk value = AddressOutpoints proto
//! ```
//!
//! ONE chunked record per address, rewritten on change. Same pattern as
//! the v3 chunked `OUTPOINT_TO_RUNES`. The v2 layout (`/outpoint/byaddress/
//! <addr>/length` + per-slot pointers) was discarded — it produced
//! `O(outpoints-for-this-address-touched-in-this-block)` writes, which
//! was the dominant per-block write source on mainnet exchange-rich
//! blocks.
//!
//! ## Determinism contract
//!
//! The chunk's `outpoints` list MUST be sorted by `(txid_le, vout)`
//! ascending. View callers iterate the list in order and need
//! byte-equal RPC responses across nodes; a wrong sort order would
//! manifest as a divergent JSON response between two nodes running this
//! feature.
//!
//! ## Cost model
//!
//! Each address-touching block costs ONE chunk write per address
//! touched, regardless of how many of that address's outpoints were
//! added or removed in the block. The write itself is
//! `O(current-chunk-size)` because we re-serialize the whole proto. For
//! exchange-grade hot wallets with hundreds of outpoints, a future
//! optimization could shard the chunk by buckets — left as deferred
//! work behind a TODO in `write_address_index`.
//!
//! ## Parallel-read future work
//!
//! The view-side per-outpoint balance-sheet reads in
//! `protorune::view::protorunes_by_address` are perfectly parallel —
//! each is an independent storage read with no side effects. A future
//! PR will dispatch them concurrently via `metashrew_core::view::spawn`
//! once that wrapper ships in the host. This PR keeps the synchronous
//! loop — the speedup will be a one-line change at that point.

#![cfg(feature = "address-indexing")]

use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::Transaction;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::address::Payload;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use prost::Message;
use protorune_support::network::to_address_str;
use protorune_support::proto;
use std::collections::{BTreeMap, BTreeSet};

/// `/v3/addr/<address-bytes>` — root of the per-address chunked
/// outpoint index. Each leaf is a `set_chunk`-encoded
/// `AddressOutpoints` protobuf.
pub static ADDRESS_OUTPOINTS: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/v3/addr/"));

/// Load the current `AddressOutpoints` chunk for an address, returning
/// `None` if no chunk has been written.
pub fn load_chunk(address: &[u8]) -> Option<proto::protorune::AddressOutpoints> {
    let ptr = ADDRESS_OUTPOINTS.select(&address.to_vec());
    let bytes = ptr.get_chunk()?;
    proto::protorune::AddressOutpoints::decode(bytes.as_slice()).ok()
}

/// Sort key for an `Outpoint` proto: `(txid_bytes, vout)` ascending.
///
/// The txid bytes are stored as little-endian (the standard wire
/// format), so plain `Vec<u8>::cmp` on the raw 32 bytes gives the
/// canonical order documented in the determinism contract.
fn sort_key(o: &proto::protorune::Outpoint) -> (Vec<u8>, u32) {
    (o.txid.clone(), o.vout)
}

/// Sort an in-memory outpoint list by `(txid_le, vout)` ascending.
fn sort_outpoints(outpoints: &mut Vec<proto::protorune::Outpoint>) {
    outpoints.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
}

/// Write back the chunk for an address with the new outpoint set and
/// snapshot height. Caller is responsible for having sorted the
/// outpoints — `write_chunk` does NOT re-sort, because the typical
/// caller has just produced the list by `BTreeMap::values().cloned()`
/// or similar.
fn write_chunk(
    address: &[u8],
    outpoints: Vec<proto::protorune::Outpoint>,
    height: u32,
) {
    let mut ptr = ADDRESS_OUTPOINTS.select(&address.to_vec());
    let proto_chunk = proto::protorune::AddressOutpoints {
        outpoints,
        height,
    };
    let encoded = proto_chunk.encode_to_vec();
    ptr.set_chunk(&encoded);
}

/// Compute the per-block per-address diff for a transaction batch and
/// rewrite each affected address's chunk.
///
/// Adds: each output paid to a parseable address.
/// Removes: each input whose previous_output is currently listed in
/// the input-side address's chunk (we look this up by checking which
/// addresses' chunks currently contain the consumed outpoint — a
/// single lookup per input by re-walking the txdata index of
/// previous-block outputs is NOT possible in this writer, since the
/// address that previously owned the input is not part of the input
/// itself).
///
/// Instead we use the existing `OUTPOINT_TO_OUTPUT` table (written
/// unconditionally by `index_outpoints`) to recover the script_pubkey
/// of the consumed input and re-derive its address. This is the only
/// piece of state the writer reads from outside the
/// `/v3/addr/...` namespace, and it is already a per-outpoint
/// canonical write — no extra storage cost.
///
/// All writes go through the in-memory `BTreeMap<address, ...>`
/// accumulator so each address gets at most ONE chunk rewrite per
/// block regardless of how many of its outpoints were touched.
///
/// The accumulator-rewrite pattern guarantees:
///  - byte-equal output across two nodes (`BTreeMap` iteration order +
///    deterministic outpoint sort);
///  - O(addresses-touched) chunk writes per block (NOT
///    O(outpoints-touched));
///  - the post-block chunk for each address reflects the net effect
///    of the block, regardless of intra-block add/remove sequencing.
pub fn write_address_index(
    txdata: &Vec<Transaction>,
    height: u32,
    updated_addresses: &mut BTreeSet<Vec<u8>>,
) -> Result<()> {
    use crate::tables;
    use protorune_support::utils::consensus_encode;

    // Per-block diff aggregated by address — address bytes -> (adds, removes).
    let mut diff: BTreeMap<Vec<u8>, (Vec<proto::protorune::Outpoint>, Vec<proto::protorune::Outpoint>)>
        = BTreeMap::new();

    for transaction in txdata.iter() {
        let tx_id = transaction.compute_txid();
        // Outputs: add each output paid to a parseable address.
        for (index, output) in transaction.output.iter().enumerate() {
            let script_pubkey = &output.script_pubkey;
            if Payload::from_script(script_pubkey).is_err() {
                continue;
            }
            let address_str = match to_address_str(script_pubkey) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let address_bytes = address_str.into_bytes();
            let op = proto::protorune::Outpoint {
                txid: tx_id.as_byte_array().to_vec(),
                vout: index as u32,
            };
            diff.entry(address_bytes)
                .or_insert_with(|| (Vec::new(), Vec::new()))
                .0
                .push(op);
        }
        // Inputs: recover the address that previously owned each
        // consumed outpoint from OUTPOINT_TO_OUTPUT.script and queue a
        // removal under that address.
        for input in transaction.input.iter() {
            let outpoint_bytes = consensus_encode(&input.previous_output)?;
            let stored = tables::OUTPOINT_TO_OUTPUT
                .select(&outpoint_bytes)
                .get();
            if stored.len() == 0 {
                // Coinbase or pre-indexing input — nothing to remove.
                continue;
            }
            let stored_output = match proto::protorune::Output::decode(stored.as_ref().as_slice()) {
                Ok(o) => o,
                Err(_) => continue,
            };
            let script_pubkey = bitcoin::ScriptBuf::from_bytes(stored_output.script);
            if Payload::from_script(&script_pubkey).is_err() {
                continue;
            }
            let address_str = match to_address_str(&script_pubkey) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let address_bytes = address_str.into_bytes();
            let op = proto::protorune::Outpoint {
                txid: input.previous_output.txid.as_byte_array().to_vec(),
                vout: input.previous_output.vout,
            };
            diff.entry(address_bytes)
                .or_insert_with(|| (Vec::new(), Vec::new()))
                .1
                .push(op);
        }
    }

    // Apply each address's diff atomically: read existing chunk,
    // union it with the per-block adds, then drop any outpoint named
    // in `removes`. Removing AFTER adding handles the same-block
    // spend case — an output paid to address X in tx-i and then
    // consumed as an input by tx-j in the same block must NOT appear
    // in X's chunk after the block applies.
    for (address, (adds, removes)) in diff.into_iter() {
        let existing = load_chunk(&address)
            .map(|c| c.outpoints)
            .unwrap_or_default();
        // Union: existing + adds, dedup'd by (txid_le, vout).
        let mut union: Vec<proto::protorune::Outpoint> = existing;
        for op in adds {
            let key = sort_key(&op);
            if !union.iter().any(|o| sort_key(o) == key) {
                union.push(op);
            }
        }
        // Removes apply to the union — same-block spend case is
        // handled because the just-added output is in `union` when
        // the input that consumes it queues the removal.
        let remove_set: BTreeSet<(Vec<u8>, u32)> = removes.iter().map(sort_key).collect();
        let mut next: Vec<proto::protorune::Outpoint> = union
            .into_iter()
            .filter(|o| !remove_set.contains(&sort_key(o)))
            .collect();
        sort_outpoints(&mut next);
        write_chunk(&address, next, height);
        // Cache-invalidation hook: when the upstream caller also has
        // --features cache, the writer's view of "updated this block"
        // is the union of inputs and outputs, which is exactly the
        // set we just rewrote.
        updated_addresses.insert(address);
    }
    Ok(())
}
