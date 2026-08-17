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

/// Convex weighting of a qualifying action: `weight = size^1.5`.
///
/// `size` is however many units of `token` came *out* of a successful call to
/// `target` opcode `op` — LP minted, tokens received, whatever the app emits.
/// Reverts earn nothing because [`OP_STATUS`] gates the result.
///
/// Convexity is the whole point. Because `f(x) = x^1.5` is convex, one action of
/// size `n` earns strictly more than `k` actions of size `n/k`, so lumping beats
/// dripping without any minimum-size constant to pick, argue about, or have
/// gamed. The shape of the curve does the work a threshold would do badly.
pub fn convex_out(
    target: (u128, u128),
    op: u128,
    token: (u128, u128),
) -> Vec<u128> {
    vec![
        // qualify: target matches, opcode matches, and the call succeeded
        OP_TARGET_BLOCK, OP_PUSH, target.0, OP_EQ,
        OP_TARGET_TX, OP_PUSH, target.1, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, op, OP_EQ, OP_AND,
        OP_STATUS, OP_AND,
        // size = units of `token` emitted
        OP_OUT_AMOUNT, token.0, token.1,
        // size^1.5 = size * sqrt(size)
        OP_DUP, OP_SQRT, OP_MUL,
        // gate by the qualify flag (0 or 1)
        OP_MUL,
    ]
}

/// Linear weighting at `num/den` of emitted `token`, for a RATE-mode rule.
///
/// This is the payout curve half of the design: [`convex_out`] and this differ
/// only in the reduce step, which is why the "normalized split versus fixed rate"
/// question is a parameter and not an architecture.
pub fn linear_out(
    target: (u128, u128),
    op: u128,
    token: (u128, u128),
    num: u128,
    den: u128,
) -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, target.0, OP_EQ,
        OP_TARGET_TX, OP_PUSH, target.1, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, op, OP_EQ, OP_AND,
        OP_STATUS, OP_AND,
        OP_OUT_AMOUNT, token.0, token.1,
        // size * num / den, via MULDIV so the intermediate is 256-bit
        OP_PUSH, num, OP_PUSH, den, OP_MULDIV,
        OP_MUL,
    ]
}
