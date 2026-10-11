//! Bounds of the per-block bytecode cache (`vm::utils::cached_decompress`).
//! The indexing suites never reach the caps, so they are exercised here:
//! every lookup must return the right bytes, the cache must stay within its
//! entry/byte caps, and an oversized (zip-bomb-shaped) payload must be served
//! but never retained.
use crate::vm::instance::reset_vm_cache;
use crate::vm::utils::{binary_cache_stats, cached_decompress};
use alkanes_support::gz::compress;
use anyhow::Result;
use std::sync::Arc;
use wasm_bindgen_test::wasm_bindgen_test;

fn payload(i: usize) -> (Arc<Vec<u8>>, Vec<u8>) {
    let raw: Vec<u8> = (0..(512 + i % 97)).map(|j| ((i * 31 + j * 7) % 251) as u8).collect();
    (Arc::new(compress(raw.clone()).unwrap()), raw)
}

#[wasm_bindgen_test]
fn binary_cache_stays_bounded_and_correct() -> Result<()> {
    reset_vm_cache();
    let mut kept = vec![];
    for i in 0..1_100 {
        let (p, raw) = payload(i);
        assert_eq!(cached_decompress(&p)?.as_ref(), &raw, "payload {}", i);
        // A second lookup of the same Arc (identity fast path) and of an equal
        // but distinct Arc (content path) must return the same bytes.
        assert_eq!(cached_decompress(&p)?.as_ref(), &raw);
        assert_eq!(cached_decompress(&Arc::new(p.as_ref().clone()))?.as_ref(), &raw);
        let (entries, bytes) = binary_cache_stats();
        assert!(entries <= 512, "entries {} over cap", entries);
        assert!(bytes <= 128 * 1024 * 1024, "bytes {} over cap", bytes);
        kept.push(p);
    }
    // After resets, earlier payloads still decompress correctly (re-cached).
    for i in [0usize, 1, 511, 512, 1_099] {
        let (_, raw) = payload(i);
        assert_eq!(cached_decompress(&kept[i])?.as_ref(), &raw);
    }
    reset_vm_cache();
    assert_eq!(binary_cache_stats(), (0, 0));
    Ok(())
}

#[wasm_bindgen_test]
fn oversized_payload_is_served_but_not_cached() -> Result<()> {
    reset_vm_cache();
    let bomb = vec![0u8; 5 * 1024 * 1024];
    let p = Arc::new(compress(bomb.clone())?);
    assert!(p.len() < 64 * 1024, "compresses tiny");
    let before = binary_cache_stats();
    assert_eq!(cached_decompress(&p)?.len(), bomb.len());
    assert_eq!(binary_cache_stats(), before, "oversized payload must not be retained");
    Ok(())
}
