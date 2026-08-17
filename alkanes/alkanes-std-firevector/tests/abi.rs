//! Tests for the 12:0 call surface.
//!
//! These pin two things that are easy to get wrong and expensive to get wrong:
//! the failure policy (which inputs degrade to zero weights and which are
//! errors), and the cellpack wire layout callers must produce. The layout test
//! runs against the *real* `CellpackDecode` from alkanes-support rather than a
//! local mirror, because a mirror would agree with my misunderstanding.

#![allow(clippy::field_reassign_with_default)]

use alkanes_std_firevector::abi::*;
use alkanes_std_firevector::codec::{encode_item, encode_items};
use alkanes_std_firevector::programs;
use alkanes_std_firevector::vm::*;

const DIESEL: (u128, u128) = (2, 0);
const MINT: u128 = 77;
const POOL: (u128, u128) = (4, 1778);

fn mint_item() -> Item {
    Item {
        target_block: DIESEL.0,
        target_tx: DIESEL.1,
        opcode: MINT,
        status: 1,
        ..Default::default()
    }
}

fn lp_item(amount: u128) -> Item {
    Item {
        target_block: POOL.0,
        target_tx: POOL.1,
        opcode: 1,
        status: 1,
        outgoing: vec![(POOL.0, POOL.1, amount)],
        ..Default::default()
    }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}

// ---------------------------------------------------------------------------
// source discriminant
// ---------------------------------------------------------------------------

#[test]
fn source_words_map_to_sources() {
    assert_eq!(source_from_word(SOURCE_PREV_BLOCK), Ok(Source::PrevBlock));
    assert_eq!(source_from_word(SOURCE_SAME_BLOCK), Ok(Source::SameBlock));
}

#[test]
fn unknown_source_is_rejected_and_names_the_value() {
    assert_eq!(source_from_word(2), Err(AbiError::UnknownSource(2)));
    let msg = AbiError::UnknownSource(7).to_string();
    assert!(msg.contains('7'), "error should name the bad value: {msg}");
}

// ---------------------------------------------------------------------------
// weight packing
// ---------------------------------------------------------------------------

#[test]
fn weights_roundtrip_as_little_endian_u128() {
    let w = vec![0u128, 1, 77, u128::MAX, 1 << 100];
    let packed = pack_weights(&w);
    assert_eq!(packed.len(), w.len() * 16);
    assert_eq!(unpack_weights(&packed), Some(w));
}

#[test]
fn packing_is_bare_with_no_length_prefix() {
    // Callers read weights at fixed 16-byte offsets, so there must be no header
    // in front of the first weight.
    let packed = pack_weights(&[1]);
    assert_eq!(packed.len(), 16);
    assert_eq!(packed[0], 1);
    assert!(packed[1..].iter().all(|b| *b == 0));
}

#[test]
fn unpacking_rejects_a_partial_trailing_weight() {
    // Silently dropping a truncated tail would turn a transport bug into a
    // plausible-looking short weight vector.
    assert!(unpack_weights(&[0u8; 17]).is_none());
    assert!(unpack_weights(&[0u8; 15]).is_none());
    assert_eq!(unpack_weights(&[]), Some(vec![]));
}

// ---------------------------------------------------------------------------
// evaluate — failure policy
// ---------------------------------------------------------------------------

#[test]
fn evaluate_weights_every_mint_equally_under_the_identity_vector() {
    let items: Vec<Item> = (0..64).map(|_| mint_item()).collect();
    let table = encode_items(&items);
    let out = evaluate_call(SOURCE_PREV_BLOCK, &programs::identity(), &table).unwrap();
    let weights = unpack_weights(&out).unwrap();
    assert_eq!(weights.len(), 64);
    assert!(weights.iter().all(|w| *w == 1));
}

#[test]
fn evaluate_returns_one_weight_per_item_including_non_qualifying_ones() {
    // The vector must be positionally aligned with the item table — a caller
    // matching weights to items by index would otherwise silently misattribute.
    let mut a = mint_item();
    a.opcode = 78; // does not qualify
    let items = vec![a, mint_item(), lp_item(5)];
    let out = evaluate_call(SOURCE_PREV_BLOCK, &programs::identity(), &encode_items(&items)).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![0, 1, 0]);
}

#[test]
fn an_invalid_program_yields_zero_weights_rather_than_an_error() {
    // Programs are governance data and therefore attacker-influenced. "Nothing
    // qualifies" is a safe outcome the caller can fall back from; an error is
    // something a caller might propagate into a block failure.
    let items = vec![mint_item(), mint_item()];
    let out = evaluate_call(SOURCE_PREV_BLOCK, &[OP_ADD], &encode_items(&items))
        .expect("invalid program must not be an error");
    assert_eq!(unpack_weights(&out).unwrap(), vec![0, 0]);
}

#[test]
fn a_program_using_unavailable_facts_yields_zeros_for_its_source() {
    // The convex vector reads STATUS and OUT_AMOUNT, so it is meaningless as a
    // same-block rule. It must produce zeros there, and real weights as a
    // prev-block rule, from the identical call.
    let items = vec![lp_item(1_000_000)];
    let table = encode_items(&items);
    let p = programs::convex_out(POOL, 1, POOL);

    let same = evaluate_call(SOURCE_SAME_BLOCK, &p, &table).unwrap();
    assert_eq!(unpack_weights(&same).unwrap(), vec![0]);

    let prev = evaluate_call(SOURCE_PREV_BLOCK, &p, &table).unwrap();
    assert!(unpack_weights(&prev).unwrap()[0] > 0);
}

#[test]
fn a_malformed_item_table_is_an_error() {
    // The indexer builds the table itself, so garbage here means the indexer is
    // broken. Returning zeros would hide that behind plausible output.
    let p = programs::identity();
    assert_eq!(
        evaluate_call(SOURCE_PREV_BLOCK, &p, &[]),
        Err(AbiError::MalformedItemTable)
    );
    // Claims one item, supplies no words for it.
    assert_eq!(
        evaluate_call(SOURCE_PREV_BLOCK, &p, &[1]),
        Err(AbiError::MalformedItemTable)
    );
    // Hostile count.
    assert_eq!(
        evaluate_call(SOURCE_PREV_BLOCK, &p, &[u128::MAX]),
        Err(AbiError::MalformedItemTable)
    );
}

#[test]
fn an_unknown_source_is_an_error_before_anything_else_happens() {
    let items = encode_items(&[mint_item()]);
    assert_eq!(
        evaluate_call(9, &programs::identity(), &items),
        Err(AbiError::UnknownSource(9))
    );
}

#[test]
fn evaluate_handles_an_empty_item_table() {
    let out = evaluate_call(SOURCE_PREV_BLOCK, &programs::identity(), &[0]).unwrap();
    assert_eq!(out.len(), 0);
    assert_eq!(unpack_weights(&out), Some(vec![]));
}

// ---------------------------------------------------------------------------
// simulate
// ---------------------------------------------------------------------------

#[test]
fn simulate_returns_a_single_weight() {
    let out = simulate_call(
        SOURCE_PREV_BLOCK,
        &programs::convex_out(POOL, 1, POOL),
        &encode_item(&lp_item(1_000_000)),
    )
    .unwrap();
    let w = unpack_weights(&out).unwrap();
    assert_eq!(w.len(), 1);
    // 1e6^1.5 = 1e9
    assert_eq!(w[0], 1_000_000_000);
}

#[test]
fn simulate_takes_a_bare_item_not_a_table() {
    // Easy caller mistake: passing an encode_items() table to simulate. The
    // leading count word shifts every field by one, so the result must NOT be a
    // plausible-looking weight — either a decode error or a different answer,
    // never the same number the correct call would give.
    let p = programs::identity();
    let bare = simulate_call(SOURCE_PREV_BLOCK, &p, &encode_item(&mint_item())).unwrap();
    assert_eq!(unpack_weights(&bare).unwrap(), vec![1]);

    match simulate_call(SOURCE_PREV_BLOCK, &p, &encode_items(&[mint_item()])) {
        Err(AbiError::MalformedItem) => {}
        Ok(out) => assert_ne!(
            unpack_weights(&out).unwrap(),
            vec![1],
            "a table passed to simulate must not weigh as if it were an item"
        ),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn simulate_zeroes_an_invalid_program_but_errors_on_a_malformed_item() {
    let item = encode_item(&mint_item());
    let out = simulate_call(SOURCE_PREV_BLOCK, &[OP_ADD], &item).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![0]);

    assert_eq!(
        simulate_call(SOURCE_PREV_BLOCK, &programs::identity(), &[]),
        Err(AbiError::MalformedItem)
    );
}

#[test]
fn simulate_agrees_with_evaluate_for_the_same_item() {
    // A wallet's preview must match what the indexer will actually compute.
    let p = programs::convex_out(POOL, 1, POOL);
    for amount in [0u128, 1, 999, 1_000_000, u128::MAX] {
        let it = lp_item(amount);
        let sim = unpack_weights(&simulate_call(SOURCE_PREV_BLOCK, &p, &encode_item(&it)).unwrap())
            .unwrap()[0];
        let ev = unpack_weights(
            &evaluate_call(SOURCE_PREV_BLOCK, &p, &encode_items(&[it])).unwrap(),
        )
        .unwrap()[0];
        assert_eq!(sim, ev, "amount {amount}");
    }
}

// ---------------------------------------------------------------------------
// validate / disassemble views
// ---------------------------------------------------------------------------

#[test]
fn validate_view_reports_steps_for_a_good_program() {
    let text = validate_call(SOURCE_PREV_BLOCK, &programs::identity());
    assert!(text.starts_with("valid"), "got {text:?}");
    assert!(text.contains("11 steps/item"), "got {text:?}");
}

#[test]
fn validate_view_explains_a_bad_program() {
    let text = validate_call(SOURCE_PREV_BLOCK, &[OP_ADD]);
    assert!(text.starts_with("INVALID"), "got {text:?}");
    assert!(text.contains("underflow"), "got {text:?}");
}

#[test]
fn validate_view_names_the_unavailable_fact_for_a_sameblock_rule() {
    let text = validate_call(SOURCE_SAME_BLOCK, &programs::convex_out(POOL, 1, POOL));
    assert!(text.starts_with("INVALID"), "got {text:?}");
    assert!(text.contains("unavailable"), "got {text:?}");
}

#[test]
fn validate_view_rejects_an_unknown_source() {
    let text = validate_call(42, &programs::identity());
    assert!(text.starts_with("INVALID"), "got {text:?}");
}

#[test]
fn disassemble_view_is_total() {
    // Never an error, because whoever is debugging a malformed vector needs to
    // see it.
    assert!(disassemble_call(&programs::identity()).contains("TARGET_BLOCK"));
    assert!(disassemble_call(&[]).is_empty());
    assert!(disassemble_call(&[u128::MAX]).contains("unknown opcode"));
}

// ---------------------------------------------------------------------------
// adoption gate
// ---------------------------------------------------------------------------

#[test]
fn adoption_gate_accepts_a_vector_that_weights_something() {
    let probes = vec![mint_item()];
    assert!(is_adoptable(
        &programs::identity(),
        SOURCE_PREV_BLOCK,
        &probes
    ));
}

#[test]
fn adoption_gate_rejects_a_vector_that_weights_nothing() {
    // Well-formed but useless. Adopting this would silently halt emission to
    // everybody, which is a governance accident worth blocking at the setter.
    let never = vec![OP_PUSH, 0];
    assert!(validate(&never, Source::PrevBlock).is_ok());
    assert!(!is_adoptable(&never, SOURCE_PREV_BLOCK, &[mint_item()]));

    // Also rejects a vector whose predicate simply never matches the probes.
    let other_app = programs::convex_out((4, 9999), 1, POOL);
    assert!(!is_adoptable(&other_app, SOURCE_PREV_BLOCK, &[lp_item(10)]));
}

#[test]
fn adoption_gate_rejects_invalid_programs_and_sources() {
    assert!(!is_adoptable(&[OP_ADD], SOURCE_PREV_BLOCK, &[mint_item()]));
    assert!(!is_adoptable(&programs::identity(), 99, &[mint_item()]));
    assert!(!is_adoptable(&programs::identity(), SOURCE_PREV_BLOCK, &[]));
}

// ---------------------------------------------------------------------------
// cellpack wire layout — against the real decoder
// ---------------------------------------------------------------------------

#[test]
fn evaluate_cellpack_layout_matches_the_generated_decoder() {
    use alkanes_support::wit_abi::CellpackDecode;

    // What a caller must put in a cellpack for opcode 1, after the opcode word:
    //   source, program_len, program..., items_len, items...
    let program = programs::identity();
    let items = encode_items(&[mint_item()]);

    let mut inputs: Vec<u128> = vec![SOURCE_PREV_BLOCK];
    inputs.push(program.len() as u128);
    inputs.extend_from_slice(&program);
    inputs.push(items.len() as u128);
    inputs.extend_from_slice(&items);

    // Decode exactly as the generated dispatch does.
    let mut off = 0usize;
    let source = inputs[off];
    off += 1;
    let decoded_program = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    let decoded_items = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();

    assert_eq!(source, SOURCE_PREV_BLOCK);
    assert_eq!(decoded_program, program);
    assert_eq!(decoded_items, items);
    assert_eq!(off, inputs.len(), "layout must consume the whole cellpack");

    // And the round trip produces the weight we expect end to end.
    let out = evaluate_call(source, &decoded_program, &decoded_items).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![1]);
}

#[test]
fn simulate_cellpack_layout_matches_the_generated_decoder() {
    use alkanes_support::wit_abi::CellpackDecode;

    let program = programs::convex_out(POOL, 1, POOL);
    let item = encode_item(&lp_item(10_000));

    let mut inputs: Vec<u128> = vec![SOURCE_PREV_BLOCK];
    inputs.push(program.len() as u128);
    inputs.extend_from_slice(&program);
    inputs.push(item.len() as u128);
    inputs.extend_from_slice(&item);

    let mut off = 0usize;
    let source = inputs[off];
    off += 1;
    let p = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    let i = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    assert_eq!(off, inputs.len());

    let w = unpack_weights(&simulate_call(source, &p, &i).unwrap()).unwrap();
    // 10_000^1.5 = 1_000_000
    assert_eq!(w, vec![1_000_000]);
}

// ---------------------------------------------------------------------------
// totality at the ABI boundary
// ---------------------------------------------------------------------------

#[test]
fn abi_entry_points_never_panic_on_random_words() {
    let mut rng = Lcg(0xC0FF_EE01);
    for _ in 0..10_000 {
        let plen = rng.below(20) as usize;
        let program: Vec<u128> = (0..plen).map(|_| rng.below(120) as u128).collect();
        let tlen = rng.below(30) as usize;
        let table: Vec<u128> = (0..tlen)
            .map(|_| match rng.below(6) {
                0 => u128::MAX,
                1 => rng.next() as u128,
                _ => rng.below(4) as u128,
            })
            .collect();
        let source = rng.below(4) as u128;

        let _ = evaluate_call(source, &program, &table);
        let _ = simulate_call(source, &program, &table);
        let _ = validate_call(source, &program);
        let _ = disassemble_call(&program);
        let _ = is_adoptable(&program, source, &[mint_item()]);
    }
}

#[test]
fn evaluate_never_panics_on_truncated_item_tables() {
    let table = encode_items(&[mint_item(), lp_item(7), mint_item()]);
    for cut in 0..=table.len() {
        let _ = evaluate_call(SOURCE_PREV_BLOCK, &programs::identity(), &table[..cut]);
    }
}

// ---------------------------------------------------------------------------
// error strings
// ---------------------------------------------------------------------------

#[test]
fn every_abi_error_renders_something_actionable() {
    assert!(AbiError::MalformedItemTable
        .to_string()
        .contains("item table"));
    assert!(AbiError::MalformedItem.to_string().contains("item"));
    let unknown = AbiError::UnknownSource(3).to_string();
    assert!(unknown.contains('3'), "got {unknown:?}");
    assert!(unknown.contains("prev-block"), "got {unknown:?}");
    assert!(unknown.contains("same-block"), "got {unknown:?}");
}

// ---------------------------------------------------------------------------
// The governance setter's gate
// ---------------------------------------------------------------------------

#[test]
fn programs_roundtrip_through_storage_packing() {
    for p in [
        programs::identity(),
        programs::legacy_winner_takes_all(),
        programs::convex_out(POOL, 1, POOL),
    ] {
        assert_eq!(unpack_program(&pack_program(&p)), Some(p));
    }
}

#[test]
fn program_packing_rejects_a_partial_word() {
    // The indexer reads these bytes back directly, so a truncated tail must be
    // refused rather than decoded into a shorter, still-runnable program.
    assert!(unpack_program(&[0u8; 17]).is_none());
    assert!(unpack_program(&[0u8; 15]).is_none());
    assert!(unpack_program(&[]).is_none());
}

#[test]
fn the_adoption_gate_accepts_the_vectors_we_actually_ship() {
    check_adoptable(SOURCE_SAME_BLOCK, &programs::identity()).expect("identity must be adoptable");
    check_adoptable(SOURCE_SAME_BLOCK, &programs::legacy_winner_takes_all())
        .expect("legacy rule must be adoptable");
    check_adoptable(SOURCE_SAME_BLOCK, &programs::first_n_mints(3))
        .expect("first-n must be adoptable");
    check_adoptable(SOURCE_PREV_BLOCK, &programs::convex_out(POOL, 1, POOL))
        .expect("convex vector must be adoptable");
}

#[test]
fn the_adoption_gate_refuses_a_vector_that_would_halt_emission() {
    // Well-formed, runs fine, pays nobody. Installing it would stop DIESEL
    // emission entirely and look like a policy rather than a mistake — so the
    // setter refuses it.
    let never = vec![OP_PUSH, 0];
    assert!(validate(&never, Source::SameBlock).is_ok());
    let err = check_adoptable(SOURCE_SAME_BLOCK, &never).unwrap_err();
    assert!(err.contains("halt emission"), "got {err:?}");
}

#[test]
fn the_adoption_gate_refuses_invalid_programs_and_sources() {
    assert!(check_adoptable(SOURCE_SAME_BLOCK, &[OP_ADD]).is_err());
    assert!(check_adoptable(SOURCE_SAME_BLOCK, &[]).is_err());
    assert!(check_adoptable(9, &programs::identity()).is_err());
    // STATUS cannot be answered same-block, so the source and the program
    // disagree and the write is refused rather than silently weighing zero.
    assert!(check_adoptable(SOURCE_SAME_BLOCK, &programs::convex_out(POOL, 1, POOL)).is_err());
}

#[test]
fn the_adoption_gate_is_total() {
    let mut rng = Lcg(0xADD0_9A7E);
    for _ in 0..5_000 {
        let len = rng.below(20) as usize;
        let p: Vec<u128> = (0..len).map(|_| rng.below(120) as u128).collect();
        let _ = check_adoptable(rng.below(4) as u128, &p);
    }
}
