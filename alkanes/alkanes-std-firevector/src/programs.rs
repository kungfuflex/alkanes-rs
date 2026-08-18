//! Canonical weightmap programs.
//!
//! [`identity`] is the one that matters operationally: it reproduces today's
//! DIESEL emission split exactly. FIREVECTOR activates with this vector loaded,
//! so the upgrade is a behavioural no-op and every subsequent change is a
//! governance write rather than a fork.

use crate::vm::*;

/// The current "communist mint", as a vector.
///
/// Weight 1 for any transaction calling DIESEL (2:0) opcode 77, weight 0 for
/// everything else. Since every qualifying item has equal weight, a normalized
/// split degenerates to `block_reward / N` — precisely what
/// `create_upgraded_mint_transfer` computes today.
///
/// Fourteen words. This is the entirety of current emission policy.
pub fn identity() -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
    ]
}

/// The **pre-upgrade** DIESEL rule: the first mint in the block takes everything.
///
/// This is what `create_mint_transfer` did before `/upgrade_initialized` was
/// set — `observe_mint` claimed `/seen/<height>` and every later mint in the
/// block reverted with "already minted for block", leaving one winner with the
/// whole reward.
///
/// Two honest caveats, because "backwards compatible" should mean something
/// precise:
///
/// - **Distribution matches, arithmetic does not quite.** The winner here
///   receives `block_reward - diesel_fee`, because that is what the deployed
///   contract computes and FIREVECTOR does not modify it. The legacy contract
///   paid `current_block_reward()` with no fee carve-out.
/// - **Losers earn nothing rather than reverting.** Under this vector a
///   non-winning mint is paid zero; under the legacy contract it reverted, which
///   is observably different in the trace and in where its alkanes land
///   (refund pointer rather than pointer).
///
/// So this reproduces the *economics* of the old rule, not its execution. It
/// exists mostly to demonstrate that the rule is expressible at all — which it
/// only is because of [`OP_MINT_RANK`].
pub fn legacy_winner_takes_all() -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
        // ...and be the first mint in the block
        OP_MINT_RANK, OP_PUSH, 0, OP_EQ, OP_AND,
    ]
}

/// Reward the first `n` mints of a block equally, the generalisation of
/// [`legacy_winner_takes_all`].
///
/// Included because it is the obvious thing governance reaches for once
/// [`OP_MINT_RANK`] exists, and because writing it out shows the rank fact is
/// worth more than the one legacy vector that motivated it.
pub fn first_n_mints(n: u128) -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
        OP_MINT_RANK, OP_PUSH, n, OP_LT, OP_AND,
    ]
}

/// Weight linearly by the amount declared on the qualifying prior action:
/// `weight = arg * num / den`.
///
/// `arg` indexes the qualifying call's cellpack inputs — for a swap declaring
/// `amount_in` as its first argument after the opcode, that is `arg = 0`. There
/// is no filtering here for the target or the opcode, because that lives in the
/// weightmap header's `Qualifier` and the indexer has already applied it: an
/// item that did not qualify never reaches the program.
///
/// The declared amount is trustworthy for the reason described on
/// [`OP_PRIOR_INPUT`] — a caller who overstates it makes their own swap revert,
/// and a reverted swap drains the transaction's fuel so their mint dies too.
///
/// `MULDIV` rather than `MUL` then `DIV` so the intermediate is 256-bit and a
/// large `num` cannot saturate before the division brings it back into range.
pub fn linear_on_prior(arg: u128, num: u128, den: u128) -> Vec<u128> {
    vec![
        OP_PRIOR_INPUT, arg,
        OP_PUSH, num, OP_PUSH, den, OP_MULDIV,
    ]
}

/// Convex weighting of the qualifying action: `weight = size^1.5`.
///
/// Convexity is the whole point. Because `f(x) = x^1.5` is convex, one action of
/// size `n` earns strictly more than `k` actions of size `n/k`, so lumping beats
/// dripping without any minimum-size constant to pick, argue about, or have
/// gamed. The shape of the curve does the work a threshold would do badly.
pub fn convex_on_prior(arg: u128) -> Vec<u128> {
    vec![
        OP_PRIOR_INPUT, arg,
        // size^1.5 = size * sqrt(size)
        OP_DUP, OP_SQRT, OP_MUL,
    ]
}

/// Weight linearly by the units of `token` that actually **arrived** in the mint
/// protostone's `incoming_alkanes`. RATE mode only.
///
/// Stronger evidence than [`linear_on_prior`]: tokens reach a protostone only if
/// the protostone that sent them succeeded *and* passed reconciliation, so this
/// cannot be forged by a malformed header or a reconcile failure — the two ways
/// the fuel-drain guarantee can be sidestepped.
///
/// The trade-off is that it measures *possession*, not activity. `mint()`
/// forwards its incoming alkanes straight back out, so the same balance can be
/// presented again in a later transaction of the same block. Prefer
/// [`linear_on_prior`] when the intent is to reward doing something rather than
/// holding something.
pub fn linear_on_incoming(token: (u128, u128), num: u128, den: u128) -> Vec<u128> {
    vec![
        OP_INCOMING_AMOUNT, token.0, token.1,
        OP_PUSH, num, OP_PUSH, den, OP_MULDIV,
    ]
}

/// Sum-of-hinges: `f(x) = Σ slope_i * max(0, x - breakpoint_i)`, where `load` is
/// the instruction sequence that pushes `x`.
///
/// A builder rather than an opcode, because the machine already expresses this:
/// saturating `SUB` *is* `max(0, x - bp)`, so the piecewise shape needs no
/// branching at all.
///
/// `load` is re-emitted once per hinge rather than the value being duplicated,
/// and that is load-bearing. The only stack ops are `DUP`/`SWAP`/`DROP`, none of
/// which can reach past the top two slots — so an accumulator-plus-`DUP` form
/// cannot get at `x` once it is buried, and the alternative of leaving every
/// partial term on the stack would cap the builder at `MAX_STACK` hinges.
/// Reloading holds depth at 2 regardless of length, so with a two-word loader
/// this is 8 words per breakpoint and ~500 of them fit inside
/// `MAX_PROGRAM_WORDS` — more resolution than emission policy can use.
///
/// Breakpoints should be ascending; nothing enforces it, and out-of-order entries
/// simply sum to a different (still total, still deterministic) function.
///
/// ```text
/// piecewise_hinges(&[OP_PRIOR_INPUT, 0], &[(10, 1), (20, 2)])
///   =>  x < 10        : 0
///       10 <= x < 20  : (x-10)
///       x >= 20       : (x-10) + 2*(x-20)
/// ```
pub fn piecewise_hinges(load: &[u128], breakpoints: &[(u128, u128)]) -> Vec<u128> {
    let mut out = vec![OP_PUSH, 0];
    for (bp, slope) in breakpoints {
        out.extend_from_slice(load);
        out.extend([OP_PUSH, *bp, OP_SUB, OP_PUSH, *slope, OP_MUL, OP_ADD]);
    }
    out
}
