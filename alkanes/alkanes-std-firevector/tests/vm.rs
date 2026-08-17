//! FIREVECTOR VM test suite.
//!
//! Organised around the two properties the design actually depends on:
//! totality (a validated program cannot fault, and an *unvalidated* one still
//! cannot panic) and determinism. Everything else — arithmetic semantics,
//! convexity, the identity vector — is checked because consensus depends on the
//! exact numbers, not just on "it runs".

use alkanes_std_firevector::codec::{decode_items, encode_items};
use alkanes_std_firevector::programs;
use alkanes_std_firevector::vm::*;
use alkanes_std_firevector::{disassemble, evaluate_table};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DIESEL: (u128, u128) = (2, 0);
const MINT: u128 = 77;

/// A DIESEL mint item, as the identity vector expects to see one.
fn mint_item() -> Item {
    Item {
        target_block: DIESEL.0,
        target_tx: DIESEL.1,
        opcode: MINT,
        status: 1,
        txindex: 3,
        pstone_index: 1,
        height: 950_000,
        ..Default::default()
    }
}

/// A successful call to `target` opcode `op` that emitted `amount` of `token`.
fn out_item(target: (u128, u128), op: u128, token: (u128, u128), amount: u128) -> Item {
    Item {
        target_block: target.0,
        target_tx: target.1,
        opcode: op,
        status: 1,
        outgoing: vec![(token.0, token.1, amount)],
        ..Default::default()
    }
}

/// Validate and run, asserting the program is well-formed. Use in tests that are
/// about semantics rather than about validation.
fn run(program: &[u128], item: &Item) -> u128 {
    let steps = validate(program, Source::PrevBlock)
        .unwrap_or_else(|e| panic!("program should validate: {e}"));
    eval(program, item, steps)
}

/// A tiny deterministic LCG. Tests must not depend on a real RNG — a flaky
/// consensus test is worse than no test.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

// ---------------------------------------------------------------------------
// The identity vector — activation behaviour
// ---------------------------------------------------------------------------

#[test]
fn identity_vector_is_fourteen_words() {
    // Not vanity: this vector is quoted verbatim in the spec and in the
    // governance channel. If it changes size, the published description is wrong.
    assert_eq!(programs::identity().len(), 14);
}

#[test]
fn identity_vector_validates_under_both_sources() {
    // The identity vector must be legal as a SameBlock rule, because that is how
    // it reproduces today's semantics: today's count is a parse-only pre-pass.
    assert!(validate(&programs::identity(), Source::SameBlock).is_ok());
    assert!(validate(&programs::identity(), Source::PrevBlock).is_ok());
}

#[test]
fn identity_vector_weights_a_mint_one() {
    assert_eq!(run(&programs::identity(), &mint_item()), 1);
}

#[test]
fn identity_vector_rejects_non_mints() {
    let p = programs::identity();

    // Right contract, wrong opcode.
    let mut it = mint_item();
    it.opcode = 78;
    assert_eq!(run(&p, &it), 0);

    // Right opcode, wrong contract.
    let mut it = mint_item();
    it.target_tx = 1;
    assert_eq!(run(&p, &it), 0);

    let mut it = mint_item();
    it.target_block = 4;
    assert_eq!(run(&p, &it), 0);
}

#[test]
fn identity_vector_gives_every_mint_equal_weight() {
    // This is what makes activation a no-op: equal weights mean a normalized
    // split degenerates to reward/N, which is exactly
    // create_upgraded_mint_transfer's `(block_reward - diesel_fee) / total_mints`.
    let p = programs::identity();
    let items: Vec<Item> = (0..500u128)
        .map(|i| {
            let mut it = mint_item();
            it.txindex = i;
            it.height = 950_000 + i;
            it
        })
        .collect();
    let weights = evaluate_table(&p, &items, Source::PrevBlock);
    assert_eq!(weights.len(), 500);
    assert!(weights.iter().all(|w| *w == 1));
}

#[test]
fn identity_vector_ignores_status() {
    // Today's count is parse-only and does not check success, so the identity
    // vector must not either — otherwise activation would silently change
    // behaviour for reverted mints.
    let p = programs::identity();
    let mut it = mint_item();
    it.status = 0;
    assert_eq!(run(&p, &it), 1);
}

// ---------------------------------------------------------------------------
// Convexity — the property that replaces a minimum-size constant
// ---------------------------------------------------------------------------

#[test]
fn convex_curve_makes_lumping_beat_dripping() {
    let pool = (4u128, 1778u128);
    let token = (4u128, 1778u128);
    let p = programs::convex_out(pool, 1, token);

    // One action of 1_000_000 versus a thousand actions of 1_000.
    let lump = run(&p, &out_item(pool, 1, token, 1_000_000));

    let drip: u128 = (0..1000)
        .map(|_| run(&p, &out_item(pool, 1, token, 1_000)))
        .sum();

    assert!(
        lump > drip,
        "convexity must reward lumping: lump={lump} drip={drip}"
    );
}

#[test]
fn convex_curve_is_monotonic_and_superlinear() {
    let pool = (4u128, 1778u128);
    let p = programs::convex_out(pool, 1, pool);

    let mut prev = 0u128;
    for size in [1u128, 10, 100, 10_000, 1_000_000, 10_000_000_000] {
        let w = run(&p, &out_item(pool, 1, pool, size));
        assert!(w >= prev, "weight must be monotonic in size");
        prev = w;
    }

    // Doubling size must more than double weight (x^1.5 grows as 2^1.5 ≈ 2.83).
    let a = run(&p, &out_item(pool, 1, pool, 1_000_000));
    let b = run(&p, &out_item(pool, 1, pool, 2_000_000));
    assert!(b > 2 * a, "x^1.5 must be superlinear: a={a} b={b}");
}

#[test]
fn convex_curve_zeroes_reverts() {
    // The whole reason for previous-block settlement: a reverted action earns
    // nothing, so spending big into a call rigged to fail is not a free way to
    // dilute everyone else.
    let pool = (4u128, 1778u128);
    let p = programs::convex_out(pool, 1, pool);

    let mut it = out_item(pool, 1, pool, 1_000_000);
    it.status = 0;
    assert_eq!(run(&p, &it), 0);
}

#[test]
fn convex_curve_zeroes_wrong_target() {
    let pool = (4u128, 1778u128);
    let p = programs::convex_out(pool, 1, pool);
    let it = out_item((4, 9999), 1, pool, 1_000_000);
    assert_eq!(run(&p, &it), 0);
}

#[test]
fn convex_curve_saturates_rather_than_wrapping() {
    // size^1.5 on a huge size must saturate, not wrap to something small. A wrap
    // here would be a weight-inflation exploit.
    let pool = (4u128, 1778u128);
    let p = programs::convex_out(pool, 1, pool);
    let w = run(&p, &out_item(pool, 1, pool, u128::MAX));
    assert_eq!(w, u128::MAX);
}

#[test]
fn linear_curve_is_proportional() {
    let pool = (4u128, 1778u128);
    let p = programs::linear_out(pool, 1, pool, 3, 2);
    assert_eq!(run(&p, &out_item(pool, 1, pool, 1_000)), 1_500);
    assert_eq!(run(&p, &out_item(pool, 1, pool, 2_000)), 3_000);
}

// ---------------------------------------------------------------------------
// Arithmetic semantics — consensus depends on the exact numbers
// ---------------------------------------------------------------------------

fn arith(program: Vec<u128>) -> u128 {
    run(&program, &Item::default())
}

#[test]
fn add_saturates() {
    assert_eq!(arith(vec![OP_PUSH, u128::MAX, OP_PUSH, 1, OP_ADD]), u128::MAX);
    assert_eq!(arith(vec![OP_PUSH, 2, OP_PUSH, 3, OP_ADD]), 5);
}

#[test]
fn sub_saturates_at_zero() {
    // No wrapping to u128::MAX. An underflow that wrapped would be an
    // enormous-weight exploit.
    assert_eq!(arith(vec![OP_PUSH, 3, OP_PUSH, 10, OP_SUB]), 0);
    assert_eq!(arith(vec![OP_PUSH, 10, OP_PUSH, 3, OP_SUB]), 7);
}

#[test]
fn mul_saturates() {
    assert_eq!(
        arith(vec![OP_PUSH, u128::MAX, OP_PUSH, 2, OP_MUL]),
        u128::MAX
    );
    assert_eq!(arith(vec![OP_PUSH, 6, OP_PUSH, 7, OP_MUL]), 42);
}

#[test]
fn div_by_zero_is_zero_not_a_trap() {
    assert_eq!(arith(vec![OP_PUSH, 100, OP_PUSH, 0, OP_DIV]), 0);
    assert_eq!(arith(vec![OP_PUSH, 100, OP_PUSH, 7, OP_DIV]), 14);
}

#[test]
fn shifts_past_width_are_zero() {
    assert_eq!(arith(vec![OP_PUSH, 1, OP_SHL, 128]), 0);
    assert_eq!(arith(vec![OP_PUSH, u128::MAX, OP_SHR, 128]), 0);
    assert_eq!(arith(vec![OP_PUSH, u128::MAX, OP_SHR, 200]), 0);
    assert_eq!(arith(vec![OP_PUSH, 1, OP_SHL, 10]), 1024);
    assert_eq!(arith(vec![OP_PUSH, 1024, OP_SHR, 10]), 1);
}

#[test]
fn comparisons_and_logic_yield_exactly_zero_or_one() {
    for (prog, want) in [
        (vec![OP_PUSH, 1, OP_PUSH, 1, OP_EQ], 1),
        (vec![OP_PUSH, 1, OP_PUSH, 2, OP_EQ], 0),
        (vec![OP_PUSH, 1, OP_PUSH, 2, OP_NE], 1),
        (vec![OP_PUSH, 1, OP_PUSH, 2, OP_LT], 1),
        (vec![OP_PUSH, 2, OP_PUSH, 1, OP_GT], 1),
        (vec![OP_PUSH, 2, OP_PUSH, 2, OP_LE], 1),
        (vec![OP_PUSH, 2, OP_PUSH, 2, OP_GE], 1),
        (vec![OP_PUSH, 7, OP_PUSH, 9, OP_AND], 1),
        (vec![OP_PUSH, 7, OP_PUSH, 0, OP_AND], 0),
        (vec![OP_PUSH, 0, OP_PUSH, 9, OP_OR], 1),
        (vec![OP_PUSH, 0, OP_PUSH, 0, OP_OR], 0),
        (vec![OP_PUSH, 0, OP_NOT], 1),
        (vec![OP_PUSH, 5, OP_NOT], 0),
    ] {
        assert_eq!(arith(prog.clone()), want, "program {prog:?}");
    }
}

#[test]
fn select_picks_by_truthiness() {
    // c a b -> a if c != 0 else b
    assert_eq!(arith(vec![OP_PUSH, 1, OP_PUSH, 10, OP_PUSH, 20, OP_SELECT]), 10);
    assert_eq!(arith(vec![OP_PUSH, 0, OP_PUSH, 10, OP_PUSH, 20, OP_SELECT]), 20);
    assert_eq!(arith(vec![OP_PUSH, 99, OP_PUSH, 10, OP_PUSH, 20, OP_SELECT]), 10);
}

#[test]
fn stack_ops_behave() {
    assert_eq!(arith(vec![OP_PUSH, 5, OP_DUP, OP_MUL]), 25);
    // SWAP then SUB: 10 3 -> 3 10 -> 3-10 saturates to 0
    assert_eq!(arith(vec![OP_PUSH, 10, OP_PUSH, 3, OP_SWAP, OP_SUB]), 0);
    assert_eq!(arith(vec![OP_PUSH, 1, OP_PUSH, 9, OP_DROP]), 1);
}

#[test]
fn isqrt_is_exact_floor_everywhere_it_matters() {
    for n in [0u128, 1, 2, 3, 4, 8, 9, 15, 16, 99, 100, 101, 10_000, 1 << 64, u128::MAX] {
        let r = isqrt(n);
        assert!(r.saturating_mul(r) <= n, "isqrt({n})={r} too big");
        // Guard the upper bound without overflowing at u128::MAX.
        if let Some(next) = r.checked_add(1) {
            if let Some(sq) = next.checked_mul(next) {
                assert!(sq > n, "isqrt({n})={r} too small");
            }
        }
    }
}

#[test]
fn isqrt_matches_bruteforce_on_small_values() {
    for n in 0u128..4096 {
        let want = (0u128..)
            .take_while(|k| k * k <= n)
            .last()
            .unwrap_or(0);
        assert_eq!(isqrt(n), want, "isqrt({n})");
    }
}

#[test]
fn muldiv_uses_a_wide_intermediate() {
    // The point of MULDIV: a*b overflows u128, but a*b/c does not, and a naive
    // saturating mul would return u128::MAX / c instead of the right answer.
    let a = u128::MAX;
    assert_eq!(mul_div_floor(a, 2, 4), a / 2);
    assert_eq!(mul_div_floor(a, 2, 2), a);
    assert_eq!(mul_div_floor(a, 3, 3), a);

    // A wrapping implementation would produce garbage here.
    let big = 1u128 << 100;
    assert_eq!(mul_div_floor(big, big, big), big);
}

#[test]
fn muldiv_agrees_with_plain_arithmetic_when_no_overflow() {
    let mut rng = Lcg(0xF12E_0001);
    for _ in 0..2000 {
        let a = rng.next() as u128;
        let b = rng.next() as u128;
        let c = (rng.next() as u128).max(1);
        assert_eq!(mul_div_floor(a, b, c), (a * b) / c, "a={a} b={b} c={c}");
    }
}

#[test]
fn muldiv_by_zero_is_zero() {
    assert_eq!(mul_div_floor(10, 10, 0), 0);
    assert_eq!(mul_div_floor(u128::MAX, u128::MAX, 0), 0);
}

#[test]
fn muldiv_saturates_when_the_true_quotient_exceeds_u128() {
    // MAX * MAX / 1 does not fit; saturate rather than wrap.
    assert_eq!(mul_div_floor(u128::MAX, u128::MAX, 1), u128::MAX);
}

/// Values chosen so `a * b` genuinely overflows u128 and the 256-bit long
/// division runs. The fast `checked_mul` path would otherwise hide every bug in
/// it, which is exactly what the random test above does.
#[test]
fn muldiv_slow_path_is_exercised_and_exact() {
    // Sanity: these all overflow, so the fallback is what answers.
    for (a, b) in [
        (1u128 << 127, 1u128 << 127),
        (1u128 << 100, 1u128 << 100),
        (u128::MAX, u128::MAX),
        (u128::MAX, 3),
    ] {
        assert!(a.checked_mul(b).is_none(), "case must overflow: {a} {b}");
    }

    assert_eq!(mul_div_floor(1u128 << 127, 1u128 << 127, 1u128 << 127), 1u128 << 127);
    // 2^200 / 2^127 = 2^73
    assert_eq!(mul_div_floor(1u128 << 100, 1u128 << 100, 1u128 << 127), 1u128 << 73);
    // 2^227 / 2^127 = 2^100
    assert_eq!(mul_div_floor(1u128 << 127, 1u128 << 100, 1u128 << 127), 1u128 << 100);
    assert_eq!(mul_div_floor(u128::MAX, u128::MAX, u128::MAX), u128::MAX);
    assert_eq!(mul_div_floor(u128::MAX, 3, 3), u128::MAX);
    assert_eq!(mul_div_floor(u128::MAX, 4, 2), u128::MAX); // saturates
}

#[test]
fn muldiv_identities_hold_across_the_overflow_boundary() {
    // Two identities that must hold exactly regardless of which path is taken:
    //   a*b/b == a   and   a*b/a == b
    let mut rng = Lcg(0x9E37_79B9);
    for _ in 0..3000 {
        // Force operands large enough that many pairs overflow.
        let a = ((rng.next() as u128) << 64) | rng.next() as u128;
        let b = ((rng.next() as u128) << 64) | rng.next() as u128;
        if a == 0 || b == 0 {
            continue;
        }
        assert_eq!(mul_div_floor(a, b, b), a, "a*b/b != a for a={a} b={b}");
        assert_eq!(mul_div_floor(a, b, a), b, "a*b/a != b for a={a} b={b}");
    }
}

#[test]
fn muldiv_slow_path_matches_a_reference_on_wide_operands() {
    // Independent reference: compute a*b as a 256-bit pair, then divide by a
    // power of two via shifting, which needs no division at all. Any discrepancy
    // is a bug in the long division rather than in both implementations.
    fn mul_hi_lo(a: u128, b: u128) -> (u128, u128) {
        const M: u128 = u64::MAX as u128;
        let (al, ah) = (a & M, a >> 64);
        let (bl, bh) = (b & M, b >> 64);
        let ll = al * bl;
        let lh = al * bh;
        let hl = ah * bl;
        let hh = ah * bh;
        let mid = (ll >> 64) + (lh & M) + (hl & M);
        (hh + (lh >> 64) + (hl >> 64) + (mid >> 64), (ll & M) | (mid & M) << 64)
    }
    fn shr256(hi: u128, lo: u128, n: u32) -> Option<u128> {
        // Result must fit u128 for the comparison to be meaningful.
        if n == 0 {
            return if hi == 0 { Some(lo) } else { None };
        }
        if n >= 128 {
            let v = hi >> (n - 128);
            return Some(v);
        }
        if hi >> n != 0 {
            return None; // quotient exceeds u128
        }
        Some((lo >> n) | (hi << (128 - n)))
    }

    let mut rng = Lcg(0x1234_5678);
    let mut slow_path_hits = 0;
    for _ in 0..3000 {
        let a = ((rng.next() as u128) << 64) | rng.next() as u128;
        let b = ((rng.next() as u128) << 64) | rng.next() as u128;
        let shift = 1 + rng.below(200) as u32;
        let c = if shift >= 128 {
            // Keep c a power of two inside u128.
            1u128 << 127
        } else {
            1u128 << shift
        };
        let eff_shift = c.trailing_zeros();

        if a.checked_mul(b).is_none() {
            slow_path_hits += 1;
        }
        let (hi, lo) = mul_hi_lo(a, b);
        if let Some(want) = shr256(hi, lo, eff_shift) {
            assert_eq!(
                mul_div_floor(a, b, c),
                want,
                "a={a} b={b} c=2^{eff_shift}"
            );
        }
    }
    assert!(
        slow_path_hits > 1000,
        "expected the wide path to dominate, hit {slow_path_hits}"
    );
}

// ---------------------------------------------------------------------------
// Fact loads
// ---------------------------------------------------------------------------

#[test]
fn input_out_of_range_is_zero_not_a_panic() {
    // Calldata is zero-padded to 15-byte chunks on the wire, so arity is not
    // recoverable and out-of-range reads are normal, not exceptional.
    let mut it = Item::default();
    it.inputs = vec![77, 5];

    assert_eq!(arith_on(&it, vec![OP_INPUT, 0]), 77);
    assert_eq!(arith_on(&it, vec![OP_INPUT, 1]), 5);
    assert_eq!(arith_on(&it, vec![OP_INPUT, 2]), 0);
    assert_eq!(arith_on(&it, vec![OP_INPUT, 9_999]), 0);
    assert_eq!(arith_on(&it, vec![OP_INPUT, u128::MAX]), 0);
}

#[test]
fn trailing_zero_inputs_are_indistinguishable_from_padding() {
    // Documented limitation, asserted so nobody later writes a vector that
    // depends on telling them apart.
    let mut padded = Item::default();
    padded.inputs = vec![77, 0, 0];
    let mut bare = Item::default();
    bare.inputs = vec![77];

    let p = vec![OP_INPUT, 1];
    assert_eq!(arith_on(&padded, p.clone()), arith_on(&bare, p));
}

fn arith_on(item: &Item, program: Vec<u128>) -> u128 {
    run(&program, item)
}

#[test]
fn amounts_sum_across_transfers_of_the_same_alkane() {
    let mut it = Item::default();
    it.incoming = vec![(32, 0, 100), (4, 1776, 7), (32, 0, 250)];
    it.outgoing = vec![(4, 1778, 42)];

    assert_eq!(arith_on(&it, vec![OP_IN_AMOUNT, 32, 0]), 350);
    assert_eq!(arith_on(&it, vec![OP_IN_AMOUNT, 4, 1776]), 7);
    assert_eq!(arith_on(&it, vec![OP_OUT_AMOUNT, 4, 1778]), 42);
}

#[test]
fn absent_alkane_amount_is_zero() {
    let it = Item::default();
    assert_eq!(arith_on(&it, vec![OP_IN_AMOUNT, 32, 0]), 0);
    assert_eq!(arith_on(&it, vec![OP_OUT_AMOUNT, 999, 999]), 0);
}

#[test]
fn amount_sums_saturate() {
    let mut it = Item::default();
    it.incoming = vec![(1, 1, u128::MAX), (1, 1, u128::MAX)];
    assert_eq!(arith_on(&it, vec![OP_IN_AMOUNT, 1, 1]), u128::MAX);
}

#[test]
fn positional_facts_load() {
    let it = mint_item();
    assert_eq!(arith_on(&it, vec![OP_TARGET_BLOCK]), 2);
    assert_eq!(arith_on(&it, vec![OP_TARGET_TX]), 0);
    assert_eq!(arith_on(&it, vec![OP_OPCODE]), 77);
    assert_eq!(arith_on(&it, vec![OP_STATUS]), 1);
    assert_eq!(arith_on(&it, vec![OP_TXINDEX]), 3);
    assert_eq!(arith_on(&it, vec![OP_PSTONE_INDEX]), 1);
    assert_eq!(arith_on(&it, vec![OP_HEIGHT]), 950_000);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_program() {
    assert_eq!(validate(&[], Source::PrevBlock), Err(ValidateError::Empty));
}

#[test]
fn rejects_unknown_opcode() {
    assert!(matches!(
        validate(&[OP_PUSH, 1, 9_999_999], Source::PrevBlock),
        Err(ValidateError::UnknownOpcode { .. })
    ));
}

#[test]
fn rejects_truncated_immediate() {
    // PUSH with no operand word after it.
    assert!(matches!(
        validate(&[OP_PUSH], Source::PrevBlock),
        Err(ValidateError::TruncatedImmediate { .. })
    ));
    // IN_AMOUNT needs two words, gets one.
    assert!(matches!(
        validate(&[OP_IN_AMOUNT, 32], Source::PrevBlock),
        Err(ValidateError::TruncatedImmediate { .. })
    ));
}

#[test]
fn rejects_stack_underflow() {
    assert!(matches!(
        validate(&[OP_ADD], Source::PrevBlock),
        Err(ValidateError::StackUnderflow { .. })
    ));
    assert!(matches!(
        validate(&[OP_PUSH, 1, OP_MULDIV], Source::PrevBlock),
        Err(ValidateError::StackUnderflow { .. })
    ));
}

#[test]
fn rejects_stack_overflow() {
    // MAX_STACK+1 pushes.
    let mut p = Vec::new();
    for _ in 0..(MAX_STACK + 1) {
        p.push(OP_PUSH);
        p.push(1);
    }
    assert!(matches!(
        validate(&p, Source::PrevBlock),
        Err(ValidateError::StackOverflow { .. })
    ));
}

#[test]
fn accepts_exactly_max_stack() {
    let mut p = Vec::new();
    for _ in 0..MAX_STACK {
        p.push(OP_PUSH);
        p.push(1);
    }
    // Collapse back down to a single residue.
    for _ in 0..(MAX_STACK - 1) {
        p.push(OP_ADD);
    }
    assert!(validate(&p, Source::PrevBlock).is_ok());
}

#[test]
fn rejects_program_that_does_not_leave_exactly_one_value() {
    // Leaves two.
    assert!(matches!(
        validate(&[OP_PUSH, 1, OP_PUSH, 2], Source::PrevBlock),
        Err(ValidateError::BadResidue { depth: 2 })
    ));
    // Leaves none.
    assert!(matches!(
        validate(&[OP_PUSH, 1, OP_DROP], Source::PrevBlock),
        Err(ValidateError::BadResidue { depth: 0 })
    ));
}

#[test]
fn rejects_oversized_program() {
    let p = vec![OP_PUSH; MAX_PROGRAM_WORDS + 1];
    assert!(matches!(
        validate(&p, Source::PrevBlock),
        Err(ValidateError::TooLong { .. })
    ));
}

#[test]
fn rejects_unavailable_facts_in_a_sameblock_rule() {
    // STATUS and OUT_AMOUNT cannot be answered before execution. Failing loudly
    // at validation is the point: a vector reading a fact that cannot exist is a
    // bug governance should see before the block, not a silent zero after it.
    assert!(matches!(
        validate(&[OP_STATUS], Source::SameBlock),
        Err(ValidateError::FactUnavailable { .. })
    ));
    assert!(matches!(
        validate(&[OP_OUT_AMOUNT, 4, 1778], Source::SameBlock),
        Err(ValidateError::FactUnavailable { .. })
    ));

    // Both are fine for a PrevBlock rule.
    assert!(validate(&[OP_STATUS], Source::PrevBlock).is_ok());
    assert!(validate(&[OP_OUT_AMOUNT, 4, 1778], Source::PrevBlock).is_ok());
}

#[test]
fn in_amount_is_available_to_both_sources() {
    // Incoming amounts ARE recoverable pre-execution: they are what the spent
    // UTXOs carried, readable from the balance index. Only success and outputs
    // require execution.
    assert!(validate(&[OP_IN_AMOUNT, 32, 0], Source::SameBlock).is_ok());
    assert!(validate(&[OP_IN_AMOUNT, 32, 0], Source::PrevBlock).is_ok());
}

#[test]
fn convex_program_is_rejected_as_a_sameblock_rule() {
    // It reads STATUS and OUT_AMOUNT, so it is only meaningful with previous-block
    // settlement. This is the type system saying so.
    let p = programs::convex_out((4, 1778), 1, (4, 1778));
    assert!(validate(&p, Source::PrevBlock).is_ok());
    assert!(matches!(
        validate(&p, Source::SameBlock),
        Err(ValidateError::FactUnavailable { .. })
    ));
}

#[test]
fn reported_step_count_matches_instructions_executed() {
    let p = programs::identity();
    let steps = validate(&p, Source::PrevBlock).unwrap();
    // 14 words: 11 opcodes + 3 immediates.
    assert_eq!(steps, 11);
    // One step short must starve rather than mis-evaluate.
    assert_eq!(eval(&p, &mint_item(), steps), 1);
    assert_eq!(eval(&p, &mint_item(), steps - 1), 0);
}

// ---------------------------------------------------------------------------
// Totality — the h=953281 lesson, as tests
// ---------------------------------------------------------------------------

#[test]
fn eval_never_panics_on_random_programs() {
    // The real guarantee: eval is infallible for ANY word sequence, validated or
    // not. A malformed weightmap must degrade to weight 0, never to a panic that
    // takes an indexer offline.
    let opcodes: Vec<u128> = (0u128..=90).collect();
    let mut rng = Lcg(0xDEAD_BEEF);

    for _ in 0..20_000 {
        let len = 1 + rng.below(24) as usize;
        let prog: Vec<u128> = (0..len)
            .map(|_| {
                // Mostly real opcodes, sometimes garbage, sometimes extremes.
                match rng.below(10) {
                    0 => u128::MAX,
                    1 => rng.next() as u128,
                    _ => opcodes[rng.below(opcodes.len() as u64) as usize],
                }
            })
            .collect();

        let item = Item {
            target_block: rng.next() as u128,
            target_tx: rng.next() as u128,
            opcode: rng.below(200) as u128,
            inputs: vec![rng.next() as u128, 0, u128::MAX],
            status: rng.below(2) as u128,
            incoming: vec![(2, 0, rng.next() as u128)],
            outgoing: vec![(4, 1778, u128::MAX)],
            txindex: rng.next() as u128,
            pstone_index: rng.below(8) as u128,
            height: 950_000,
        };

        // Must return, must not panic, must respect the budget.
        let _ = eval(&prog, &item, 4096);
    }
}

#[test]
fn eval_never_panics_on_adversarial_shapes() {
    let item = mint_item();
    let cases: Vec<Vec<u128>> = vec![
        vec![],
        vec![OP_PUSH],
        vec![OP_IN_AMOUNT],
        vec![OP_IN_AMOUNT, 1],
        vec![OP_OUT_AMOUNT, 1],
        vec![OP_ADD],
        vec![OP_SELECT],
        vec![OP_MULDIV],
        vec![OP_DROP],
        vec![OP_SWAP],
        vec![OP_SQRT],
        vec![u128::MAX],
        vec![0],
        vec![OP_SHL],
        vec![OP_SHR],
        vec![OP_PUSH, 1, OP_SHL],
        vec![OP_INPUT],
        // Deep stack pressure, unvalidated.
        vec![OP_PUSH, 1].repeat(MAX_STACK * 4),
    ];
    for p in cases {
        let _ = eval(&p, &item, 100_000);
    }
}

#[test]
fn budget_exhaustion_yields_zero_not_a_hang() {
    // A long valid program starved of budget must return 0 promptly.
    let mut p = vec![OP_PUSH, 1];
    for _ in 0..1000 {
        p.push(OP_PUSH);
        p.push(1);
        p.push(OP_ADD);
    }
    assert!(validate(&p, Source::PrevBlock).is_ok());
    assert_eq!(eval(&p, &Item::default(), 10), 0);
}

#[test]
fn validate_never_panics_on_random_input() {
    let mut rng = Lcg(0x5EED_1234);
    for _ in 0..20_000 {
        let len = rng.below(40) as usize;
        let prog: Vec<u128> = (0..len).map(|_| rng.below(120) as u128).collect();
        let _ = validate(&prog, Source::PrevBlock);
        let _ = validate(&prog, Source::SameBlock);
    }
}

#[test]
fn invalid_program_yields_all_zero_weights_not_an_error() {
    // evaluate_table must degrade, because the caller's only safe response to a
    // bad weightmap is "nothing qualifies, fall back to the identity vector".
    let items = vec![mint_item(), mint_item()];
    let weights = evaluate_table(&[OP_ADD], &items, Source::PrevBlock);
    assert_eq!(weights, vec![0, 0]);
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn evaluation_is_deterministic() {
    let p = programs::convex_out((4, 1778), 1, (4, 1778));
    let it = out_item((4, 1778), 1, (4, 1778), 123_456_789);
    let first = run(&p, &it);
    for _ in 0..100 {
        assert_eq!(run(&p, &it), first);
    }
}

#[test]
fn evaluation_does_not_depend_on_item_order() {
    // Weights are per-item and independent; any cross-item dependence would make
    // the block's result order-sensitive and therefore a consensus hazard.
    let p = programs::convex_out((4, 1778), 1, (4, 1778));
    let a = out_item((4, 1778), 1, (4, 1778), 10);
    let b = out_item((4, 1778), 1, (4, 1778), 20);
    let c = out_item((4, 1778), 1, (4, 1778), 30);

    let fwd = evaluate_table(&p, &[a.clone(), b.clone(), c.clone()], Source::PrevBlock);
    let mut rev = evaluate_table(&p, &[c, b, a], Source::PrevBlock);
    rev.reverse();
    assert_eq!(fwd, rev);
}

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

#[test]
fn item_table_roundtrips() {
    let items = vec![
        mint_item(),
        out_item((4, 1778), 1, (4, 1778), 999),
        Item {
            target_block: 4,
            target_tx: 1776,
            opcode: 5,
            inputs: vec![1, 2, 3],
            status: 0,
            incoming: vec![(32, 0, 5), (4, 1776, 6)],
            outgoing: vec![(4, 1778, 7)],
            txindex: 12,
            pstone_index: 2,
            height: 950_123,
        },
    ];
    let encoded = encode_items(&items);
    let decoded = decode_items(&encoded).expect("must decode");
    assert_eq!(decoded, items);
}

#[test]
fn codec_rejects_truncation_instead_of_panicking() {
    let items = vec![mint_item(), out_item((4, 1778), 1, (4, 1778), 1)];
    let encoded = encode_items(&items);
    for cut in 1..encoded.len() {
        // Any prefix must either decode to something coherent or return None.
        let _ = decode_items(&encoded[..cut]);
    }
    assert!(decode_items(&[]).is_none());
}

#[test]
fn codec_rejects_absurd_lengths_without_allocating() {
    // A hostile count word must not drive a giant allocation.
    assert!(decode_items(&[u128::MAX]).is_none());
    assert!(decode_items(&[1, 0, 0, 0, 0, 0, 0, 0, u128::MAX]).is_none());
}

// ---------------------------------------------------------------------------
// Disassembly
// ---------------------------------------------------------------------------

#[test]
fn identity_vector_disassembles_readably() {
    let text = disassemble(&programs::identity());
    for expected in [
        "TARGET_BLOCK",
        "PUSH 2",
        "EQ",
        "TARGET_TX",
        "PUSH 0",
        "AND",
        "OPCODE",
        "PUSH 77",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}

#[test]
fn disassembly_renders_alkane_ids_as_block_colon_tx() {
    let text = disassemble(&[OP_OUT_AMOUNT, 4, 1778]);
    assert!(text.contains("OUT_AMOUNT 4:1778"), "got:\n{text}");
}

#[test]
fn disassembly_marks_malformed_programs_instead_of_failing() {
    assert!(disassemble(&[9_999_999]).contains("<unknown opcode 9999999>"));
    assert!(disassemble(&[OP_PUSH]).contains("<truncated immediates>"));
}

#[test]
fn disassembly_never_panics_on_random_input() {
    let mut rng = Lcg(0xAB1E_0007);
    for _ in 0..5_000 {
        let len = rng.below(30) as usize;
        let prog: Vec<u128> = (0..len).map(|_| rng.below(120) as u128).collect();
        let _ = disassemble(&prog);
    }
}
