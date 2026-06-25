//! Regression test for the auditor's #F1 finding (indexer-halt poison tx).
//!
//! Pre-fix behavior: a protostone whose calldata decodes to fewer than 2
//! varints (e.g. a 15-byte continued-LEB payload `[0x80;14, 0x01]`) would
//! reach `&v[0..2]` inside `Cellpack::try_from` and PANIC with "range
//! out of bounds". The slice expression is evaluated before the
//! `try_from`'s `?`, so the panic fires before any error can propagate.
//! Combined with metashrew's panic-stops-block behavior, a single
//! ~37-byte mineable OP_RETURN would halt every alkanes indexer
//! network-wide until a patch + reindex.
//!
//! Post-fix behavior: a length guard at the head of `Cellpack::try_from`
//! returns Err instead of panicking. The dispatcher in
//! `crates/alkanes/src/message.rs` already treats Err from this
//! `try_from` as "skip protostone, refund, continue" — so no behavior
//! change for valid txs, only the malformed poison becomes a graceful
//! refund instead of a network-wide halt.
//!
//! These tests drive `Cellpack::try_from` and `decode_varint_list` from
//! their real crates with NO auditor source. They fail (panic) on the
//! pre-fix code and pass on the fixed code.

use alkanes_support::cellpack::Cellpack;
use protorune_support::utils::decode_varint_list;
use std::io::Cursor;
use std::panic;

#[test]
fn f1_cellpack_try_from_returns_err_on_length_1_input() {
    let single = vec![0x42u128];
    let result = panic::catch_unwind(|| Cellpack::try_from(single));
    match result {
        Ok(Ok(_)) => panic!("Cellpack::try_from(vec![ONE]) unexpectedly succeeded"),
        Ok(Err(e)) => {
            eprintln!("FIX VERIFIED: length-1 input returns Err({})", e);
        }
        Err(_) => {
            panic!(
                "REGRESSION — length-1 input PANICKED instead of returning Err. \
                 The cellpack.rs length guard has been removed."
            );
        }
    }
}

#[test]
fn f1_cellpack_try_from_returns_err_on_empty_input() {
    let empty: Vec<u128> = vec![];
    let result = panic::catch_unwind(|| Cellpack::try_from(empty));
    match result {
        Ok(Ok(_)) => panic!("Cellpack::try_from(vec![]) unexpectedly succeeded"),
        Ok(Err(_)) => { /* good */ }
        Err(_) => panic!("REGRESSION — empty input PANICKED instead of returning Err."),
    }
}

#[test]
fn f1_auditor_poison_payload_decodes_to_single_varint() {
    // Auditor's exact payload: 14× 0x80 (continuation bytes) + 0x01
    // (terminator). 15 bytes of continued LEB encodes to ONE u128.
    let mut poison: Vec<u8> = vec![0x80; 14];
    poison.push(0x01);
    assert_eq!(poison.len(), 15, "poison payload is exactly 15 bytes");

    let decoded = decode_varint_list(&mut Cursor::new(poison.clone()))
        .expect("LEB decode should succeed — well-formed continued LEB");
    eprintln!(
        "CONFIRMED: 15-byte payload {:02x?} decodes to {} varint(s): {:?}",
        &poison, decoded.len(), decoded
    );
    assert_eq!(decoded.len(), 1, "15-byte continued LEB → exactly 1 varint");
}

#[test]
fn f1_end_to_end_decode_then_cellpack_returns_err() {
    // The actual indexer path: decode_varint_list(calldata)?.try_into()?
    // Pre-fix: panics. Post-fix: returns Err which the dispatcher gracefully
    // handles as "skip protostone, refund, continue".
    let mut poison: Vec<u8> = vec![0x80; 14];
    poison.push(0x01);
    let decoded = decode_varint_list(&mut Cursor::new(poison)).unwrap();
    assert_eq!(decoded.len(), 1);

    let result = panic::catch_unwind(|| {
        let cellpack: anyhow::Result<Cellpack> = decoded.try_into();
        cellpack
    });
    match result {
        Ok(Ok(_)) => panic!("end-to-end unexpectedly produced a valid Cellpack"),
        Ok(Err(e)) => {
            eprintln!("FIX VERIFIED end-to-end: poison payload → Err({})", e);
        }
        Err(_) => {
            panic!(
                "REGRESSION — end-to-end poison payload PANICKED. \
                 The fix has been reverted somewhere in the decode → try_into chain."
            );
        }
    }
}

#[test]
fn f1_valid_two_varint_input_still_works() {
    // Sanity: valid 2-element input still produces a valid Cellpack
    // (proves the guard is specifically for length-<2, not over-restrictive).
    let two = vec![0x02u128, 0x00u128];
    let result = Cellpack::try_from(two);
    assert!(result.is_ok(), "2-element input should still produce a valid Cellpack");
    let cellpack = result.unwrap();
    assert_eq!(cellpack.target.block, 2);
    assert_eq!(cellpack.target.tx, 0);
    assert!(cellpack.inputs.is_empty());
}

#[test]
fn f1_valid_input_with_extra_inputs_still_works() {
    // Sanity: 5-element input → 2 for target, 3 for inputs.
    let five = vec![0x02u128, 0x00u128, 0xAAu128, 0xBBu128, 0xCCu128];
    let cellpack = Cellpack::try_from(five).expect("valid 5-element input");
    assert_eq!(cellpack.target.block, 2);
    assert_eq!(cellpack.target.tx, 0);
    assert_eq!(cellpack.inputs, vec![0xAA, 0xBB, 0xCC]);
}
