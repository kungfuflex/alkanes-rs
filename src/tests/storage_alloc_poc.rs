//! PoC for the unbounded eager allocation in `StorageMap::parse`
//! (crates/alkanes-support/src/storage.rs:21-24).
//!
//! `parse` reads an attacker-controlled `u32` key/value length and immediately
//! calls metashrew `consume_exact`, which does `vec![0u8; n]` BEFORE checking
//! that the cursor actually holds `n` bytes. A ~8-byte buffer claiming a
//! `key_length` of `u32::MAX` therefore forces a ~4.29 GB allocation. On the
//! wasm32 indexer (32-bit address space) that aborts the process — and since a
//! `StorageMap` is parsed from the bytes an attacker-deployed contract returns
//! (`ExtendedCallResponse` / extcall `checkpoint` blob), a single cheap tx
//! wedges the whole indexer.
//!
//! Fix: bound each length against the cursor's remaining bytes before
//! allocating, so an over-long claim becomes a graceful `Err` (a per-tx revert)
//! instead of a multi-GB allocation.
//!
//! Pre-fix, `test_storage_map_parse_huge_key_length` OOM-aborts here (RuntimeError:
//! unreachable). Post-fix it returns `Err` and the assertions pass.

use alkanes_support::storage::StorageMap;
use anyhow::Result;
use std::io::Cursor;
use wasm_bindgen_test::wasm_bindgen_test;

/// Build a StorageMap wire buffer: `entry_count` then, per entry,
/// (key_len, key_bytes, value_len, value_bytes) — all lengths u32 LE.
/// Here we emit a single entry that CLAIMS `claimed_key_len` but supplies no
/// key/value bytes, the minimal malicious payload.
fn malicious_buffer(claimed_key_len: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(&1u32.to_le_bytes()); // entry_count = 1
    b.extend(&claimed_key_len.to_le_bytes()); // key_length = huge
    // no further bytes: a correct parser must error on "not enough input"
    b
}

#[wasm_bindgen_test]
fn test_storage_map_parse_huge_key_length() -> Result<()> {
    // ~8-byte payload claiming a 4.29 GB key.
    let buf = malicious_buffer(u32::MAX);
    assert!(buf.len() < 16, "payload is tiny ({} bytes)", buf.len());

    let result = StorageMap::parse(&mut Cursor::new(buf));

    // FIXED: parse must reject the over-long length WITHOUT allocating 4.29 GB.
    assert!(
        result.is_err(),
        "INVARIANT: a length claim exceeding remaining input must be a graceful Err, \
         never an eager multi-GB allocation"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_storage_map_parse_huge_value_length() -> Result<()> {
    // Valid 1-byte key, then a value that claims u32::MAX bytes with none present.
    let mut buf = Vec::new();
    buf.extend(&1u32.to_le_bytes()); // entry_count = 1
    buf.extend(&1u32.to_le_bytes()); // key_length = 1
    buf.push(0xAB); // the 1 key byte
    buf.extend(&u32::MAX.to_le_bytes()); // value_length = huge
    assert!(buf.len() < 32);

    let result = StorageMap::parse(&mut Cursor::new(buf));
    assert!(
        result.is_err(),
        "INVARIANT: over-long value length must be a graceful Err"
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_storage_map_parse_roundtrip_still_works() -> Result<()> {
    // Regression guard: a well-formed StorageMap must still parse to its
    // serialized value (the fix must not reject legitimate payloads).
    let mut original = StorageMap::default();
    original.set(b"/counter".to_vec(), 42u128.to_le_bytes().to_vec());
    original.set(b"/name".to_vec(), b"alkane".to_vec());

    let bytes = original.serialize();
    let parsed = StorageMap::parse(&mut Cursor::new(bytes))?;
    assert_eq!(parsed, original, "roundtrip must be preserved by the fix");
    Ok(())
}
