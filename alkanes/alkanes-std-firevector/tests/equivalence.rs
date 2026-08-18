//! Does the identity vector actually reproduce today's DIESEL emission?
//!
//! This file answers that as arithmetic rather than assertion. It models the
//! live payout from `create_upgraded_mint_transfer`
//! (alkanes-std-genesis-alkane-upgraded-eoa/src/lib.rs:289-320) and the
//! FIREVECTOR normalized split, and requires them to agree exactly — same
//! integer truncation, same dropped remainder, same halving.
//!
//! ```text
//!   total_mints  = number_diesel_mints()          // counted per TRANSACTION
//!   block_reward = 50e8 >> (H / 210_000)
//!   total_tx_fee = saturating(total_miner_fee - block_reward)
//!   diesel_fee   = min(block_reward / 2, total_tx_fee)
//!   value_per_mint = (block_reward - diesel_fee) / total_mints
//! ```
//!
//! FIREVECTOR replaces only the denominator: `E * w_i / sum(w)`, where `E` is
//! the same `block_reward - diesel_fee`. The fee carve-out, the supply cap, the
//! EOA-caller check and one-mint-per-tx all stay in the DIESEL contract and are
//! out of scope for the VM.

use alkanes_std_firevector::programs;
use alkanes_std_firevector::vm::*;
use alkanes_std_firevector::evaluate_table;

// ---------------------------------------------------------------------------
// A model of what mainnet does today
// ---------------------------------------------------------------------------

/// `block_reward(n)` from the mainnet `ChainConfiguration` impl.
fn block_reward(height: u64) -> u128 {
    (50e8 as u128) / (1u128 << ((height as u128) / 210_000u128))
}

/// Emission available to minters after the protocol fee carve-out.
fn emission_after_fee(height: u64, total_miner_fee: u128) -> u128 {
    let reward = block_reward(height);
    let total_tx_fee = total_miner_fee.saturating_sub(reward);
    let diesel_fee = core::cmp::min(reward / 2, total_tx_fee);
    reward - diesel_fee
}

/// What each mint receives today.
fn current_value_per_mint(height: u64, total_miner_fee: u128, total_mints: u128) -> u128 {
    emission_after_fee(height, total_miner_fee) / total_mints
}

/// What each mint receives under a FIREVECTOR normalized split.
fn firevector_claims(emission: u128, weights: &[u128]) -> Vec<u128> {
    let total: u128 = weights.iter().copied().sum();
    if total == 0 {
        return vec![0; weights.len()];
    }
    weights
        .iter()
        .map(|w| emission.saturating_mul(*w) / total)
        .collect()
}

fn mint_item(txindex: u128) -> Item {
    Item {
        target_block: 2,
        target_tx: 0,
        opcode: 77,
        txindex,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// The equivalence claim
// ---------------------------------------------------------------------------

#[test]
fn identity_vector_pays_exactly_what_mainnet_pays_today() {
    let p = programs::identity();

    // Heights spanning three halvings, fees spanning "no excess" to "far above
    // the 50% cap".
    for height in [800_000u64, 880_000, 950_000, 1_049_999, 1_050_000, 1_260_000] {
        for miner_fee in [
            0u128,
            1,
            block_reward(height),
            block_reward(height) + 1,
            block_reward(height) + block_reward(height) / 4,
            block_reward(height) * 2,
            block_reward(height) * 100,
        ] {
            for n in [1u128, 2, 3, 7, 100, 1499, 4096, 5000] {
                let items: Vec<Item> = (0..n).map(mint_item).collect();
                let weights = evaluate_table(&p, &items, Mode::Split);

                // Every mint weighs exactly 1 — that is what makes the split
                // degenerate to reward/N.
                assert!(weights.iter().all(|w| *w == 1), "h={height} n={n}");

                let emission = emission_after_fee(height, miner_fee);
                let claims = firevector_claims(emission, &weights);
                let today = current_value_per_mint(height, miner_fee, n);

                for (i, c) in claims.iter().enumerate() {
                    assert_eq!(
                        *c, today,
                        "mint {i} of {n} at h={height} fee={miner_fee}: \
                         firevector={c} mainnet={today}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_dropped_remainder_is_identical_too() {
    // Today's integer division silently never mints the remainder. Under
    // FIREVECTOR the same remainder falls out, which is what makes it available
    // to accumulate into a pool later without changing anything now.
    let p = programs::identity();
    for n in [3u128, 7, 11, 1499, 4999] {
        let items: Vec<Item> = (0..n).map(mint_item).collect();
        let weights = evaluate_table(&p, &items, Mode::Split);
        let emission = emission_after_fee(950_000, 0);

        let paid_today = current_value_per_mint(950_000, 0, n) * n;
        let paid_firevector: u128 = firevector_claims(emission, &weights).iter().sum();

        assert_eq!(paid_today, paid_firevector, "n={n}");
        assert_eq!(emission - paid_firevector, emission % n, "n={n}");
    }
}

#[test]
fn halving_boundaries_line_up() {
    // The halving is on absolute Bitcoin height, not DIESEL genesis. Pinned here
    // because the identity vector is only equivalent if the emission it divides
    // is the same one.
    assert_eq!(block_reward(839_999), 312_500_000 * 2);
    assert_eq!(block_reward(840_000), 312_500_000);
    assert_eq!(block_reward(1_049_999), 312_500_000);
    assert_eq!(block_reward(1_050_000), 156_250_000);
}

#[test]
fn a_block_with_no_mints_pays_nothing_and_does_not_divide_by_zero() {
    let p = programs::identity();
    let weights = evaluate_table(&p, &[], Mode::Split);
    assert!(weights.is_empty());
    assert_eq!(firevector_claims(emission_after_fee(950_000, 0), &weights), Vec::<u128>::new());
}

#[test]
fn non_mint_activity_does_not_dilute_the_split() {
    // A block full of unrelated alkane traffic must not change what each mint
    // receives — otherwise activation would not be a no-op on a busy block.
    let p = programs::identity();
    let mut items: Vec<Item> = (0..10).map(mint_item).collect();
    for i in 0..500u128 {
        items.push(Item {
            target_block: 4,
            target_tx: 1778,
            opcode: 5,
            txindex: 1000 + i,
            ..Default::default()
        });
    }
    let weights = evaluate_table(&p, &items, Mode::Split);
    let emission = emission_after_fee(950_000, 0);
    let claims = firevector_claims(emission, &weights);
    let today = current_value_per_mint(950_000, 0, 10);

    for (i, it) in items.iter().enumerate() {
        if it.opcode == 77 {
            assert_eq!(claims[i], today, "mint at index {i}");
        } else {
            assert_eq!(claims[i], 0, "non-mint at index {i} must earn nothing");
        }
    }
}

// ---------------------------------------------------------------------------
// The one place equivalence can be broken by the caller
// ---------------------------------------------------------------------------

#[test]
fn equivalence_requires_the_item_builder_to_count_per_transaction() {
    // `_get_number_diesel_mints` counts TRANSACTIONS, not protostones — note the
    // `break` at host_functions.rs:700, which stops scanning a tx after its first
    // 2:0/77 cellpack. A tx carrying two mint protostones therefore counts ONCE
    // today.
    //
    // The VM weighs whatever items it is handed. So if the (not yet written)
    // item builder emits one item per tag-1 protostone rather than one per
    // transaction, a double-mint tx would weigh 2 and quietly change the
    // denominator for everyone else in the block. That is a constraint on the
    // builder, and this test exists so it is discovered here rather than on
    // mainnet.
    let p = programs::identity();

    // Correct: one item per transaction.
    let per_tx: Vec<Item> = (0..3).map(mint_item).collect();
    let w_tx = evaluate_table(&p, &per_tx, Mode::Split);
    assert_eq!(w_tx.iter().sum::<u128>(), 3);

    // Wrong: tx 0 contributed two mint protostones.
    let mut per_protostone = per_tx.clone();
    per_protostone.push(Item {
        pstone_index: 1,
        ..mint_item(0)
    });
    let w_ps = evaluate_table(&p, &per_protostone, Mode::Split);
    assert_eq!(
        w_ps.iter().sum::<u128>(),
        4,
        "duplicate protostones inflate the denominator — the builder must dedup by txid"
    );

    // And the inflation is exactly the dilution everyone else suffers.
    let emission = emission_after_fee(950_000, 0);
    assert!(
        firevector_claims(emission, &w_ps)[1] < firevector_claims(emission, &w_tx)[1],
        "an honest minter must not be diluted by another tx's duplicate protostone"
    );
}

// ---------------------------------------------------------------------------
// The PRE-UPGRADE rule: first mint in the block takes everything
// ---------------------------------------------------------------------------
// Legacy `create_mint_transfer` let `observe_mint` claim `/seen/<height>`, so one
// winner took the whole reward and every later mint in the block reverted.
// Expressing that needs MINT_RANK — "am I first" is cross-item knowledge, and the
// machine is otherwise strictly per-item.

fn ranked_mint(txindex: u128, rank: u128) -> Item {
    Item {
        target_block: 2,
        target_tx: 0,
        opcode: 77,
        txindex,
        mint_rank: rank,
        ..Default::default()
    }
}

#[test]
fn legacy_vector_gives_the_whole_block_to_the_first_mint() {
    let p = programs::legacy_winner_takes_all();
    let items: Vec<Item> = (0..6).map(|i| ranked_mint(i + 1, i)).collect();
    let weights = evaluate_table(&p, &items, Mode::Split);

    assert_eq!(weights, vec![1, 0, 0, 0, 0, 0]);

    let emission = emission_after_fee(880_000, 0);
    let claims = firevector_claims(emission, &weights);
    assert_eq!(claims[0], emission, "the winner takes the entire emission");
    assert!(
        claims[1..].iter().all(|c| *c == 0),
        "every later mint earns nothing"
    );
}

#[test]
fn legacy_vector_is_expressible_only_because_of_mint_rank() {
    // Without the rank fact, the two items below are indistinguishable: same
    // target, same opcode, same everything a per-item machine can see. This test
    // exists to document WHY the fact was added, so nobody removes it as
    // redundant.
    let a = ranked_mint(1, 0);
    let mut b = ranked_mint(2, 1);
    b.txindex = a.txindex; // strip the only other distinguishing fact

    let p = programs::legacy_winner_takes_all();
    let steps = validate(&p, Mode::Split).unwrap();
    assert_eq!(eval(&p, &a, steps), 1);
    assert_eq!(eval(&p, &b, steps), 0);
}

#[test]
fn first_n_mints_generalises_the_legacy_rule() {
    let items: Vec<Item> = (0..8).map(|i| ranked_mint(i + 1, i)).collect();

    // n = 1 is exactly the legacy rule.
    assert_eq!(
        evaluate_table(&programs::first_n_mints(1), &items, Mode::Split),
        evaluate_table(&programs::legacy_winner_takes_all(), &items, Mode::Split)
    );

    // n = 3 pays the first three equally, which is the shape governance would
    // actually reach for.
    let w = evaluate_table(&programs::first_n_mints(3), &items, Mode::Split);
    assert_eq!(w, vec![1, 1, 1, 0, 0, 0, 0, 0]);

    let emission = emission_after_fee(880_000, 0);
    let claims = firevector_claims(emission, &w);
    assert_eq!(claims[0], emission / 3);
    assert_eq!(claims[3], 0);
}

#[test]
fn a_non_mint_transaction_is_never_ranked_first() {
    // mint_rank defaults to u128::MAX — "not a mint" — precisely so that a
    // default-constructed item cannot accidentally read as the block's winner.
    let p = programs::legacy_winner_takes_all();
    let steps = validate(&p, Mode::Split).unwrap();
    assert_eq!(Item::default().mint_rank, u128::MAX);
    assert_eq!(eval(&p, &Item::default(), steps), 0);
}

#[test]
fn the_legacy_vector_is_not_bit_identical_to_the_legacy_contract() {
    // Stated as a test so "backwards compatible" is not overclaimed. The legacy
    // contract paid `current_block_reward()` with NO fee carve-out; FIREVECTOR
    // divides `block_reward - diesel_fee`, because it does not modify the
    // deployed contract's arithmetic.
    let height = 880_000u64;
    let miner_fee = block_reward(height) * 3; // plenty of excess fee
    let emission = emission_after_fee(height, miner_fee);
    let legacy_contract_payout = block_reward(height);

    assert!(
        emission < legacy_contract_payout,
        "with excess fees the carve-out must bite: {emission} vs {legacy_contract_payout}"
    );

    let p = programs::legacy_winner_takes_all();
    let items = vec![ranked_mint(1, 0), ranked_mint(2, 1)];
    let claims = firevector_claims(emission, &evaluate_table(&p, &items, Mode::Split));
    assert_eq!(claims[0], emission);
    assert_ne!(
        claims[0], legacy_contract_payout,
        "distribution matches the old rule; the arithmetic deliberately does not"
    );
}
