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
        ..Default::default()
    }
}

/// A protostone that is not a DIESEL mint at all.
fn pool_item() -> Item {
    Item {
        target_block: POOL.0,
        target_tx: POOL.1,
        opcode: 1,
        ..Default::default()
    }
}

/// A DIESEL mint whose qualifying prior action declared `amount` as its first
/// cellpack input. The target/opcode match that populates `prior_inputs` lives
/// in the weightmap header's `Qualifier` and is applied by the indexer, so by
/// the time the VM sees an item only the number is left.
fn qualified_mint(amount: u128) -> Item {
    Item {
        prior_inputs: vec![amount],
        ..mint_item()
    }
}

/// A mint that actually received `amount` of `POOL` — the RATE-only fact.
fn receiving_mint(amount: u128) -> Item {
    Item {
        incoming: vec![(POOL.0, POOL.1, amount)],
        ..mint_item()
    }
}

fn split(program: Vec<u128>) -> Weightmap {
    Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        program,
    }
}

fn rate(program: Vec<u128>) -> Weightmap {
    Weightmap {
        mode: Mode::Rate,
        qualifier: None,
        rate_floor: 1_000_000,
        program,
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
// mode discriminant
// ---------------------------------------------------------------------------

#[test]
fn mode_words_map_to_modes() {
    assert_eq!(mode_from_word(MODE_RATE), Ok(Mode::Rate));
    assert_eq!(mode_from_word(MODE_SPLIT), Ok(Mode::Split));
}

#[test]
fn unknown_mode_is_rejected_and_names_the_value() {
    assert_eq!(mode_from_word(2), Err(AbiError::UnknownMode(2)));
    let msg = AbiError::UnknownMode(7).to_string();
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
    let out = evaluate_call(MODE_RATE, &programs::identity(), &table).unwrap();
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
    let items = vec![a, mint_item(), pool_item()];
    let out = evaluate_call(MODE_RATE, &programs::identity(), &encode_items(&items)).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![0, 1, 0]);
}

#[test]
fn an_invalid_program_yields_zero_weights_rather_than_an_error() {
    // Programs are governance data and therefore attacker-influenced. "Nothing
    // qualifies" is a safe outcome the caller can fall back from; an error is
    // something a caller might propagate into a block failure.
    let items = vec![mint_item(), mint_item()];
    let out = evaluate_call(MODE_RATE, &[OP_ADD], &encode_items(&items))
        .expect("invalid program must not be an error");
    assert_eq!(unpack_weights(&out).unwrap(), vec![0, 0]);
}

#[test]
fn a_program_using_unavailable_facts_yields_zeros_for_its_mode() {
    // The incoming-amount vector reads a fact that only exists once the claim is
    // executing, so it is meaningless as a SPLIT rule. It must produce zeros
    // there, and real weights as a RATE rule, from the identical call.
    let items = vec![receiving_mint(1_000_000)];
    let table = encode_items(&items);
    let p = programs::linear_on_incoming(POOL, 1, 1);

    let as_split = evaluate_call(MODE_SPLIT, &p, &table).unwrap();
    assert_eq!(unpack_weights(&as_split).unwrap(), vec![0]);

    let as_rate = evaluate_call(MODE_RATE, &p, &table).unwrap();
    assert!(unpack_weights(&as_rate).unwrap()[0] > 0);
}

#[test]
fn a_malformed_item_table_is_an_error() {
    // The indexer builds the table itself, so garbage here means the indexer is
    // broken. Returning zeros would hide that behind plausible output.
    let p = programs::identity();
    assert_eq!(
        evaluate_call(MODE_RATE, &p, &[]),
        Err(AbiError::MalformedItemTable)
    );
    // Claims one item, supplies no words for it.
    assert_eq!(
        evaluate_call(MODE_RATE, &p, &[1]),
        Err(AbiError::MalformedItemTable)
    );
    // Hostile count.
    assert_eq!(
        evaluate_call(MODE_RATE, &p, &[u128::MAX]),
        Err(AbiError::MalformedItemTable)
    );
}

#[test]
fn an_unknown_mode_is_an_error_before_anything_else_happens() {
    let items = encode_items(&[mint_item()]);
    assert_eq!(
        evaluate_call(9, &programs::identity(), &items),
        Err(AbiError::UnknownMode(9))
    );
}

#[test]
fn evaluate_handles_an_empty_item_table() {
    let out = evaluate_call(MODE_RATE, &programs::identity(), &[0]).unwrap();
    assert_eq!(out.len(), 0);
    assert_eq!(unpack_weights(&out), Some(vec![]));
}

// ---------------------------------------------------------------------------
// simulate
// ---------------------------------------------------------------------------

#[test]
fn simulate_returns_a_single_weight() {
    let out = simulate_call(
        MODE_RATE,
        &programs::convex_on_prior(0),
        &encode_item(&qualified_mint(1_000_000)),
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
    let bare = simulate_call(MODE_RATE, &p, &encode_item(&mint_item())).unwrap();
    assert_eq!(unpack_weights(&bare).unwrap(), vec![1]);

    match simulate_call(MODE_RATE, &p, &encode_items(&[mint_item()])) {
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
    let out = simulate_call(MODE_RATE, &[OP_ADD], &item).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![0]);

    assert_eq!(
        simulate_call(MODE_RATE, &programs::identity(), &[]),
        Err(AbiError::MalformedItem)
    );
}

#[test]
fn simulate_agrees_with_evaluate_for_the_same_item() {
    // A wallet's preview must match what the indexer will actually compute.
    let p = programs::convex_on_prior(0);
    for amount in [0u128, 1, 999, 1_000_000, u128::MAX] {
        let it = qualified_mint(amount);
        let sim = unpack_weights(&simulate_call(MODE_RATE, &p, &encode_item(&it)).unwrap())
            .unwrap()[0];
        let ev = unpack_weights(
            &evaluate_call(MODE_RATE, &p, &encode_items(&[it])).unwrap(),
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
    let text = validate_call(MODE_RATE, &programs::identity());
    assert!(text.starts_with("valid"), "got {text:?}");
    assert!(text.contains("11 steps/item"), "got {text:?}");
}

#[test]
fn validate_view_explains_a_bad_program() {
    let text = validate_call(MODE_RATE, &[OP_ADD]);
    assert!(text.starts_with("INVALID"), "got {text:?}");
    assert!(text.contains("underflow"), "got {text:?}");
}

#[test]
fn validate_view_names_the_unavailable_fact_for_a_split_rule() {
    let text = validate_call(MODE_SPLIT, &programs::linear_on_incoming(POOL, 1, 1));
    assert!(text.starts_with("INVALID"), "got {text:?}");
    assert!(text.contains("unavailable"), "got {text:?}");
}

#[test]
fn validate_view_rejects_an_unknown_mode() {
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
    assert!(is_adoptable(&rate(programs::identity()), &probes));
}

#[test]
fn adoption_gate_rejects_a_vector_that_weights_nothing() {
    // Well-formed but useless. Adopting this would silently halt emission to
    // everybody, which is a governance accident worth blocking at the setter.
    let never = vec![OP_PUSH, 0];
    assert!(validate(&never, Mode::Rate).is_ok());
    assert!(!is_adoptable(&rate(never), &[mint_item()]));

    // Also rejects a vector whose predicate simply never matches the probes.
    assert!(!is_adoptable(&rate(programs::identity()), &[pool_item()]));
}

#[test]
fn adoption_gate_rejects_invalid_programs_and_empty_probes() {
    assert!(!is_adoptable(&rate(vec![OP_ADD]), &[mint_item()]));
    assert!(!is_adoptable(&rate(programs::identity()), &[]));
    // A program that is invalid *for its mode* is refused just as firmly as one
    // that is invalid outright.
    assert!(!is_adoptable(
        &split(programs::linear_on_incoming(POOL, 1, 1)),
        &[receiving_mint(1_000_000)]
    ));
    // An unknown mode is no longer representable — `Weightmap::mode` is a typed
    // `Mode`, so the wire discriminant is checked once at `mode_from_word` and
    // cannot reach the gate.
}

// ---------------------------------------------------------------------------
// cellpack wire layout — against the real decoder
// ---------------------------------------------------------------------------

#[test]
fn evaluate_cellpack_layout_matches_the_generated_decoder() {
    use alkanes_support::wit_abi::CellpackDecode;

    // What a caller must put in a cellpack for opcode 1, after the opcode word:
    //   mode, program_len, program..., items_len, items...
    let program = programs::identity();
    let items = encode_items(&[mint_item()]);

    let mut inputs: Vec<u128> = vec![MODE_RATE];
    inputs.push(program.len() as u128);
    inputs.extend_from_slice(&program);
    inputs.push(items.len() as u128);
    inputs.extend_from_slice(&items);

    // Decode exactly as the generated dispatch does.
    let mut off = 0usize;
    let mode = inputs[off];
    off += 1;
    let decoded_program = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    let decoded_items = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();

    assert_eq!(mode, MODE_RATE);
    assert_eq!(decoded_program, program);
    assert_eq!(decoded_items, items);
    assert_eq!(off, inputs.len(), "layout must consume the whole cellpack");

    // And the round trip produces the weight we expect end to end.
    let out = evaluate_call(mode, &decoded_program, &decoded_items).unwrap();
    assert_eq!(unpack_weights(&out).unwrap(), vec![1]);
}

#[test]
fn simulate_cellpack_layout_matches_the_generated_decoder() {
    use alkanes_support::wit_abi::CellpackDecode;

    let program = programs::convex_on_prior(0);
    let item = encode_item(&qualified_mint(10_000));

    let mut inputs: Vec<u128> = vec![MODE_RATE];
    inputs.push(program.len() as u128);
    inputs.extend_from_slice(&program);
    inputs.push(item.len() as u128);
    inputs.extend_from_slice(&item);

    let mut off = 0usize;
    let mode = inputs[off];
    off += 1;
    let p = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    let i = <Vec<u128> as CellpackDecode>::decode_cellpack(&inputs, &mut off).unwrap();
    assert_eq!(off, inputs.len());

    let w = unpack_weights(&simulate_call(mode, &p, &i).unwrap()).unwrap();
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
        let mode_word = rng.below(4) as u128;

        let _ = evaluate_call(mode_word, &program, &table);
        let _ = simulate_call(mode_word, &program, &table);
        let _ = validate_call(mode_word, &program);
        let _ = disassemble_call(&program);

        if let Ok(mode) = mode_from_word(mode_word) {
            let w = Weightmap {
                mode,
                qualifier: None,
                rate_floor: rng.below(3) as u128,
                program,
            };
            let _ = is_adoptable(&w, &[mint_item()]);
            let _ = check_adoptable(&w);
        }
    }
}

#[test]
fn evaluate_never_panics_on_truncated_item_tables() {
    let table = encode_items(&[mint_item(), qualified_mint(7), mint_item()]);
    for cut in 0..=table.len() {
        let _ = evaluate_call(MODE_RATE, &programs::identity(), &table[..cut]);
    }
}

#[test]
fn weightmap_unpacking_never_panics_on_random_bytes() {
    // The blob comes out of storage, so a corrupt or stale one must decode to
    // None rather than to a partially-read policy.
    let mut rng = Lcg(0x5A1E_0BB1);
    for _ in 0..10_000 {
        let len = rng.below(200) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
        let _ = unpack_weightmap(&bytes);
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
    assert!(AbiError::MalformedWeightmap
        .to_string()
        .contains("weightmap"));
    let unknown = AbiError::UnknownMode(3).to_string();
    assert!(unknown.contains('3'), "got {unknown:?}");
    assert!(unknown.contains("split"), "got {unknown:?}");
    assert!(unknown.contains("rate"), "got {unknown:?}");
}

// ---------------------------------------------------------------------------
// The governance setter's gate
// ---------------------------------------------------------------------------

#[test]
fn weightmaps_roundtrip_through_storage_packing() {
    for w in [
        split(programs::identity()),
        split(programs::legacy_winner_takes_all()),
        rate(programs::convex_on_prior(0)),
        Weightmap {
            mode: Mode::Rate,
            qualifier: Some(Qualifier {
                target_block: POOL.0,
                target_tx: POOL.1,
                opcode: 1,
            }),
            rate_floor: u128::MAX,
            program: programs::linear_on_prior(0, 3, 2),
        },
    ] {
        assert_eq!(unpack_weightmap(&pack_weightmap(&w)), Some(w));
    }
}

#[test]
fn weightmap_packing_rejects_a_partial_word() {
    // The indexer reads these bytes back directly, so a truncated tail must be
    // refused rather than decoded into a shorter, still-runnable program.
    assert!(unpack_weightmap(&[0u8; 17]).is_none());
    assert!(unpack_weightmap(&[0u8; 15]).is_none());
    assert!(unpack_weightmap(&[]).is_none());
    // Whole words, but fewer than the fixed header.
    assert!(unpack_weightmap(&[0u8; 16 * 8]).is_none());
}

#[test]
fn a_bare_program_blob_is_rejected_rather_than_reinterpreted() {
    // The magic exists so a blob written by an older build — a bare program with
    // no header — is refused instead of having its first opcode silently read as
    // a mode.
    let bare: Vec<u8> = programs::identity()
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    assert!(unpack_weightmap(&bare).is_none());

    // And a declared program length that disagrees with what is present is not
    // truncated to fit.
    let mut packed = pack_weightmap(&split(programs::identity()));
    packed.truncate(packed.len() - 16);
    assert!(unpack_weightmap(&packed).is_none());
}

#[test]
fn the_adoption_gate_accepts_the_vectors_we_actually_ship() {
    check_adoptable(&split(programs::identity())).expect("identity must be adoptable");
    check_adoptable(&split(programs::legacy_winner_takes_all()))
        .expect("legacy rule must be adoptable");
    check_adoptable(&split(programs::first_n_mints(3))).expect("first-n must be adoptable");

    // The magnitude vectors are RATE vectors: SPLIT settles membership only, so
    // anything returning more than 1 belongs here rather than there.
    check_adoptable(&rate(programs::convex_on_prior(0))).expect("convex vector must be adoptable");
    check_adoptable(&rate(programs::linear_on_prior(0, 3, 2)))
        .expect("linear-on-prior must be adoptable");
    check_adoptable(&rate(programs::linear_on_incoming((32, 0), 1, 1)))
        .expect("linear-on-incoming must be adoptable");
    check_adoptable(&rate(programs::piecewise_hinges(
        &[OP_PRIOR_INPUT, 0],
        &[(10, 1), (1_000, 2)],
    )))
    .expect("piecewise hinges must be adoptable");
}

#[test]
fn the_adoption_gate_refuses_a_vector_that_would_halt_emission() {
    // Well-formed, runs fine, pays nobody. Installing it would stop DIESEL
    // emission entirely and look like a policy rather than a mistake — so the
    // setter refuses it.
    let never = vec![OP_PUSH, 0];
    assert!(validate(&never, Mode::Split).is_ok());
    let err = check_adoptable(&split(never)).unwrap_err();
    assert!(err.contains("halt emission"), "got {err:?}");
}

#[test]
fn the_adoption_gate_refuses_invalid_programs() {
    assert!(check_adoptable(&split(vec![OP_ADD])).is_err());
    assert!(check_adoptable(&split(vec![])).is_err());
    // INCOMING_AMOUNT cannot be answered during the pre-execution scan, so the
    // mode and the program disagree and the write is refused rather than
    // silently weighing zero.
    assert!(check_adoptable(&split(programs::linear_on_incoming(POOL, 1, 1))).is_err());
    // An unknown mode word cannot reach the gate at all: `Weightmap::mode` is a
    // typed `Mode`, so `mode_from_word` is the single place it is validated.
    assert!(mode_from_word(9).is_err());
}

#[test]
fn the_adoption_gate_refuses_a_rate_vector_with_no_rate_floor() {
    // rate_floor is the virtual denominator; zero would divide every payout to
    // nothing, which is the same silent emission halt as a program that weighs
    // nobody — caught at the setter for the same reason.
    let w = Weightmap {
        rate_floor: 0,
        ..rate(programs::convex_on_prior(0))
    };
    let err = check_adoptable(&w).unwrap_err();
    assert!(err.contains("rate_floor"), "got {err:?}");
}

#[test]
fn the_adoption_gate_refuses_a_split_vector_that_returns_more_than_one() {
    // SPLIT settles membership, not magnitude: the indexer clamps every non-zero
    // weight to 1. A program returning 1_000_000 there is still safe, but it is
    // not what its author meant, so the setter says so instead of quietly
    // flattening it.
    let err = check_adoptable(&split(programs::convex_on_prior(0))).unwrap_err();
    assert!(err.contains("clamps"), "got {err:?}");
    assert!(err.contains("rate mode"), "got {err:?}");
}

#[test]
fn the_adoption_gate_is_total() {
    let mut rng = Lcg(0xADD0_9A7E);
    for _ in 0..5_000 {
        let len = rng.below(20) as usize;
        let p: Vec<u128> = (0..len).map(|_| rng.below(120) as u128).collect();
        let w = Weightmap {
            mode: if rng.below(2) == 0 {
                Mode::Split
            } else {
                Mode::Rate
            },
            qualifier: None,
            rate_floor: rng.below(3) as u128,
            program: p,
        };
        let _ = check_adoptable(&w);
        let _ = unpack_weightmap(&pack_weightmap(&w));
    }
}
