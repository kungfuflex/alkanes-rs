//! Proof-of-work difficulty adjustment logic.
//! Maps to: src/pow.cpp, src/pow.h
//!
//! This module implements Bitcoin's difficulty adjustment algorithm.
//! `check_proof_of_work()` lives in `qubitcoin-consensus` (context-free check);
//! the functions here compute *new* targets based on chain history.

use qubitcoin_consensus::ConsensusParams;
use qubitcoin_primitives::arith_uint256::{uint256_to_arith, ArithUint256};

/// Get the next proof-of-work target for a block.
///
/// Port of Bitcoin Core's `GetNextWorkRequired()` from `pow.cpp`.
///
/// This is a "flattened" version: instead of walking a `CBlockIndex` chain,
/// the caller passes the data that would be read from the index.
///
/// Parameters:
/// - `last_height`: height of the current tip (`pindexLast->nHeight`).
/// - `last_bits`: compact target (`nBits`) of the current tip.
/// - `last_time`: timestamp of the current tip.
/// - `first_time`: timestamp of the first block of the retarget period,
///   i.e. the block at height `last_height - (interval - 1)`.
/// - `block_time`: timestamp of the *new* block being validated (used for
///   the testnet min-difficulty exception only).
/// - `last_non_special_bits`: on testnet, the `nBits` of the most recent
///   block that did NOT use the min-difficulty exception (i.e. walk back
///   past min-difficulty blocks). The caller resolves this from the block
///   index. On mainnet this is unused and can be set to `last_bits`.
/// - `params`: consensus parameters.
///
/// Algorithm:
/// 1. If we are **not** at a retarget boundary (`(last_height+1) % interval != 0`):
///    - On networks that allow minimum difficulty (`pow_allow_min_difficulty_blocks`):
///      if the new block's timestamp exceeds `last_time + 2 * target_spacing`,
///      return the PoW limit (easiest difficulty).
///      Otherwise, return `last_non_special_bits` — the difficulty of the
///      last block that wasn't mined under the min-difficulty exception.
///    - Otherwise return `last_bits` unchanged.
/// 2. If `pow_no_retargeting` is set (regtest), return `last_bits` unchanged.
/// 3. At a retarget boundary, compute a new target via
///    [`calculate_next_work_required`].
pub fn get_next_work_required(
    last_height: i32,
    last_bits: u32,
    last_time: u32,
    first_time: u32,
    block_time: u32,
    last_non_special_bits: u32,
    params: &ConsensusParams,
) -> u32 {
    let pow_limit = uint256_to_arith(&params.pow_limit);
    let n_proof_of_work_limit = pow_limit.get_compact(false);

    let interval = params.difficulty_adjustment_interval();

    // Only change once per difficulty adjustment interval.
    if (last_height + 1) % interval as i32 != 0 {
        if params.pow_allow_min_difficulty_blocks {
            // Special difficulty rule for testnet:
            // If the new block's timestamp is more than 2 * target_spacing
            // seconds after the previous block, allow minimum difficulty.
            if block_time as i64 > last_time as i64 + params.pow_target_spacing * 2 {
                return n_proof_of_work_limit;
            }
            // Return the difficulty of the last block that wasn't mined
            // under the min-difficulty exception. This is Bitcoin Core's
            // GetLastBlockIndex() walk resolved by the caller.
            return last_non_special_bits;
        }
        return last_bits;
    }

    // Retarget.
    calculate_next_work_required(last_bits, first_time as i64, last_time as i64, params)
}

/// Calculate the next work target given the actual timespan of the
/// previous retarget period.
///
/// Port of Bitcoin Core's `CalculateNextWorkRequired()` from `pow.cpp`.
///
/// Parameters:
/// - `current_bits`: compact target of the last block in the period.
/// - `first_block_time`: timestamp (as `i64`) of the first block of the period.
/// - `last_block_time`: timestamp (as `i64`) of the last block of the period.
/// - `params`: consensus parameters.
///
/// The actual timespan is clamped to `[target_timespan/4, target_timespan*4]`
/// before computing the new target:
///
/// ```text
///     new_target = old_target * clamped_timespan / target_timespan
/// ```
///
/// If the result exceeds `pow_limit`, it is clamped to `pow_limit`.
/// Returns the compact representation of the new target.
pub fn calculate_next_work_required(
    current_bits: u32,
    first_block_time: i64,
    last_block_time: i64,
    params: &ConsensusParams,
) -> u32 {
    // Regtest: never retarget.
    if params.pow_no_retargeting {
        return current_bits;
    }

    let target_timespan = params.pow_target_timespan;

    // Limit adjustment step.
    let mut actual_timespan = last_block_time - first_block_time;
    if actual_timespan < target_timespan / 4 {
        actual_timespan = target_timespan / 4;
    }
    if actual_timespan > target_timespan * 4 {
        actual_timespan = target_timespan * 4;
    }

    // Retarget.
    let pow_limit = uint256_to_arith(&params.pow_limit);
    let mut new_target = ArithUint256::zero();
    new_target.set_compact(current_bits);

    new_target = new_target * ArithUint256::from(actual_timespan as u64);
    new_target = new_target / ArithUint256::from(target_timespan as u64);

    if new_target > pow_limit {
        new_target = pow_limit;
    }

    new_target.get_compact(false)
}

/// Check whether a difficulty transition is permitted.
///
/// Port of Bitcoin Core's `PermittedDifficultyTransition()` from `pow.cpp`.
///
/// Returns `false` if the proof-of-work requirement specified by `new_nbits`
/// at a given `height` is not possible, given the proof-of-work on the prior
/// block as specified by `old_nbits`.
///
/// At a retarget boundary the new value must be within a factor of 4 of the
/// old value. Outside of retarget boundaries, the values must be identical.
///
/// Always returns `true` on networks with `pow_allow_min_difficulty_blocks`
/// (testnet, regtest).
pub fn permitted_difficulty_transition(
    params: &ConsensusParams,
    height: i64,
    old_nbits: u32,
    new_nbits: u32,
) -> bool {
    if params.pow_allow_min_difficulty_blocks {
        return true;
    }

    let interval = params.difficulty_adjustment_interval();

    if height % interval == 0 {
        let smallest_timespan = params.pow_target_timespan / 4;
        let largest_timespan = params.pow_target_timespan * 4;

        let pow_limit = uint256_to_arith(&params.pow_limit);

        let mut observed_new_target = ArithUint256::zero();
        observed_new_target.set_compact(new_nbits);

        // Largest (easiest) target reachable from old_nbits.
        let mut largest_difficulty_target = ArithUint256::zero();
        largest_difficulty_target.set_compact(old_nbits);
        largest_difficulty_target =
            largest_difficulty_target * ArithUint256::from(largest_timespan as u64);
        largest_difficulty_target =
            largest_difficulty_target / ArithUint256::from(params.pow_target_timespan as u64);
        if largest_difficulty_target > pow_limit {
            largest_difficulty_target = pow_limit;
        }

        // Round-trip through compact and compare.
        let mut maximum_new_target = ArithUint256::zero();
        maximum_new_target.set_compact(largest_difficulty_target.get_compact(false));
        if maximum_new_target < observed_new_target {
            return false;
        }

        // Smallest (hardest) target reachable from old_nbits.
        let mut smallest_difficulty_target = ArithUint256::zero();
        smallest_difficulty_target.set_compact(old_nbits);
        smallest_difficulty_target =
            smallest_difficulty_target * ArithUint256::from(smallest_timespan as u64);
        smallest_difficulty_target =
            smallest_difficulty_target / ArithUint256::from(params.pow_target_timespan as u64);
        if smallest_difficulty_target > pow_limit {
            smallest_difficulty_target = pow_limit;
        }

        let mut minimum_new_target = ArithUint256::zero();
        minimum_new_target.set_compact(smallest_difficulty_target.get_compact(false));
        if minimum_new_target > observed_new_target {
            return false;
        }
    } else if old_nbits != new_nbits {
        return false;
    }

    true
}

/// Check if the testnet minimum-difficulty exception should apply.
///
/// Returns `true` when `pow_allow_min_difficulty_blocks` is set and the
/// elapsed time since the previous block exceeds `2 * pow_target_spacing`.
pub fn check_minimum_difficulty(block_time: u32, last_time: u32, params: &ConsensusParams) -> bool {
    params.pow_allow_min_difficulty_blocks
        && (block_time as i64 - last_time as i64) > params.pow_target_spacing * 2
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_consensus::ConsensusParams;

    /// Helper: create mainnet-like params for PoW tests.
    fn mainnet_params() -> ConsensusParams {
        ConsensusParams::mainnet()
    }

    /// Helper: build a compact nBits from an ArithUint256.
    fn compact_from_arith(a: &ArithUint256) -> u32 {
        a.get_compact(false)
    }

    // -- 1. Non-retarget blocks return the same difficulty ------------------

    #[test]
    fn test_non_retarget_returns_same_bits() {
        let params = mainnet_params();
        // height 2014 => next block 2015, not a retarget boundary (2016-aligned).
        let last_height = 2014;
        let last_bits = 0x1d00ffff;
        let last_time = 1_000_000u32;
        let first_time = 0u32; // irrelevant
        let block_time = last_time + 600; // irrelevant

        let result = get_next_work_required(
            last_height,
            last_bits,
            last_time,
            first_time,
            block_time,
            last_bits,
            &params,
        );
        assert_eq!(result, last_bits);
    }

    #[test]
    fn test_non_retarget_mid_period() {
        let params = mainnet_params();
        // height 5000 => next block 5001, not multiple of 2016.
        let last_bits = 0x1c0fffff;
        let result = get_next_work_required(5000, last_bits, 1_000_000, 0, 1_000_600, last_bits, &params);
        assert_eq!(result, last_bits);
    }

    // -- 2. Exact target timespan => no change -----------------------------

    #[test]
    fn test_retarget_exact_timespan() {
        let params = mainnet_params();
        let last_bits = 0x1d00ffff; // genesis-era difficulty
        let target_timespan = params.pow_target_timespan;

        // first_time and last_time exactly target_timespan apart.
        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan;

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);
        // With an exact timespan, the difficulty should not change.
        assert_eq!(result, last_bits);
    }

    // -- 3. Double timespan => difficulty halves (target doubles) -----------

    #[test]
    fn test_retarget_double_timespan() {
        let params = mainnet_params();
        let last_bits = 0x1d00ffff;
        let target_timespan = params.pow_target_timespan;

        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan * 2;

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);

        // The new target should be twice the old target (difficulty halved).
        let mut old_target = ArithUint256::zero();
        old_target.set_compact(last_bits);
        let mut expected = old_target * ArithUint256::from(2u64);
        let pow_limit = uint256_to_arith(&params.pow_limit);
        if expected > pow_limit {
            expected = pow_limit;
        }
        assert_eq!(result, compact_from_arith(&expected));
    }

    // -- 4. Half timespan => difficulty doubles (target halves) -------------

    #[test]
    fn test_retarget_half_timespan() {
        let params = mainnet_params();
        let last_bits = 0x1d00ffff;
        let target_timespan = params.pow_target_timespan;

        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan / 2;

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);

        // Actual timespan = target/2 => new_target = old * (target/2) / target = old/2
        let mut old_target = ArithUint256::zero();
        old_target.set_compact(last_bits);
        let expected = old_target / ArithUint256::from(2u64);
        assert_eq!(result, compact_from_arith(&expected));
    }

    // -- 5. Clamping at 4x (timespan >> 4*target) --------------------------

    #[test]
    fn test_retarget_clamped_at_4x() {
        let params = mainnet_params();
        let last_bits = 0x1d00ffff;
        let target_timespan = params.pow_target_timespan;

        // Actual timespan = 10 * target => clamped to 4 * target.
        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan * 10;

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);

        let mut old_target = ArithUint256::zero();
        old_target.set_compact(last_bits);
        let mut expected = old_target * ArithUint256::from(4u64);
        let pow_limit = uint256_to_arith(&params.pow_limit);
        if expected > pow_limit {
            expected = pow_limit;
        }
        assert_eq!(result, compact_from_arith(&expected));
    }

    // -- 6. Clamping at 1/4x (timespan << target/4) -----------------------

    #[test]
    fn test_retarget_clamped_at_quarter() {
        let params = mainnet_params();
        let last_bits = 0x1d00ffff;
        let _target_timespan = params.pow_target_timespan;

        // Actual timespan = 1 second => clamped to target/4.
        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + 1;

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);

        let mut old_target = ArithUint256::zero();
        old_target.set_compact(last_bits);
        let expected = old_target / ArithUint256::from(4u64);
        assert_eq!(result, compact_from_arith(&expected));
    }

    // -- 7. Result does not exceed pow_limit --------------------------------

    #[test]
    fn test_retarget_does_not_exceed_pow_limit() {
        let params = mainnet_params();
        let pow_limit = uint256_to_arith(&params.pow_limit);
        let pow_limit_bits = pow_limit.get_compact(false);

        // Start at pow_limit and double the timespan => would exceed pow_limit
        // without the clamp.
        let target_timespan = params.pow_target_timespan;
        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan * 2;

        let result = calculate_next_work_required(pow_limit_bits, first_time, last_time, &params);

        // Must be clamped to pow_limit.
        let mut result_target = ArithUint256::zero();
        result_target.set_compact(result);
        assert!(result_target <= pow_limit);
        assert_eq!(result, pow_limit_bits);
    }

    // -- 8. Regtest: pow_no_retargeting returns same bits -------------------

    #[test]
    fn test_regtest_no_retarget() {
        let params = ConsensusParams::regtest();
        assert!(params.pow_no_retargeting);

        let last_bits = 0x207fffff;
        let target_timespan = params.pow_target_timespan;
        let first_time: i64 = 1_000_000;
        let last_time: i64 = first_time + target_timespan * 10; // huge timespan

        let result = calculate_next_work_required(last_bits, first_time, last_time, &params);
        assert_eq!(result, last_bits);
    }

    // -- 9. Testnet: min difficulty exception applies -----------------------

    #[test]
    fn test_testnet_min_difficulty_exception() {
        let params = ConsensusParams::testnet();
        assert!(params.pow_allow_min_difficulty_blocks);

        let pow_limit = uint256_to_arith(&params.pow_limit);
        let pow_limit_bits = pow_limit.get_compact(false);

        // Non-retarget height, and block_time > last_time + 2*spacing.
        let last_height = 100;
        let last_bits = 0x1c0fffff; // some non-trivial difficulty
        let last_time = 1_000_000u32;
        let block_time = last_time + (params.pow_target_spacing as u32) * 2 + 1;

        let result =
            get_next_work_required(last_height, last_bits, last_time, 0, block_time, last_bits, &params);
        assert_eq!(result, pow_limit_bits);
    }

    #[test]
    fn test_testnet_no_min_difficulty_when_on_time() {
        let params = ConsensusParams::testnet();
        assert!(params.pow_allow_min_difficulty_blocks);

        let last_height = 100;
        let last_bits = 0x1c0fffff;
        let last_time = 1_000_000u32;
        // Block arrives within 2 * spacing — no min-difficulty exception.
        let block_time = last_time + (params.pow_target_spacing as u32) * 2 - 1;

        let result =
            get_next_work_required(last_height, last_bits, last_time, 0, block_time, last_bits, &params);
        assert_eq!(result, last_bits);
    }

    // -- 10. check_minimum_difficulty helper --------------------------------

    #[test]
    fn test_check_minimum_difficulty() {
        let params = ConsensusParams::testnet();
        // Elapsed > 2 * spacing => true.
        let last_time = 1_000_000u32;
        let block_time = last_time + (params.pow_target_spacing as u32) * 2 + 1;
        assert!(check_minimum_difficulty(block_time, last_time, &params));

        // Elapsed == 2 * spacing => false (not strictly greater).
        let block_time_exact = last_time + (params.pow_target_spacing as u32) * 2;
        assert!(!check_minimum_difficulty(
            block_time_exact,
            last_time,
            &params
        ));

        // Mainnet: always false.
        let mainnet = mainnet_params();
        assert!(!check_minimum_difficulty(block_time, last_time, &mainnet));
    }

    // -- 11. permitted_difficulty_transition --------------------------------

    #[test]
    fn test_permitted_transition_non_retarget_must_match() {
        let params = mainnet_params();
        // Height 100 is not a retarget boundary.
        assert!(permitted_difficulty_transition(
            &params, 100, 0x1d00ffff, 0x1d00ffff
        ));
        assert!(!permitted_difficulty_transition(
            &params, 100, 0x1d00ffff, 0x1c00ffff
        ));
    }

    #[test]
    fn test_permitted_transition_at_retarget_within_range() {
        let params = mainnet_params();
        // Height 2016 is a retarget boundary.
        let old_bits = 0x1d00ffff;

        // Same bits => always ok.
        assert!(permitted_difficulty_transition(
            &params, 2016, old_bits, old_bits
        ));

        // Compute the expected 4x easier target.
        let mut old_target = ArithUint256::zero();
        old_target.set_compact(old_bits);
        let mut target_4x = old_target * ArithUint256::from(4u64);
        let pow_limit = uint256_to_arith(&params.pow_limit);
        if target_4x > pow_limit {
            target_4x = pow_limit;
        }
        let bits_4x = target_4x.get_compact(false);
        assert!(permitted_difficulty_transition(
            &params, 2016, old_bits, bits_4x
        ));

        // Compute the expected 1/4 harder target.
        let target_quarter = old_target / ArithUint256::from(4u64);
        let bits_quarter = target_quarter.get_compact(false);
        assert!(permitted_difficulty_transition(
            &params,
            2016,
            old_bits,
            bits_quarter
        ));
    }

    #[test]
    fn test_permitted_transition_testnet_always_true() {
        let params = ConsensusParams::testnet();
        // Testnet allows min difficulty, so any transition is permitted.
        assert!(permitted_difficulty_transition(
            &params, 100, 0x1d00ffff, 0x01003456
        ));
    }

    // -- 12. Full round-trip through get_next_work_required -----------------

    #[test]
    fn test_get_next_work_required_at_retarget() {
        let params = mainnet_params();
        let target_timespan = params.pow_target_timespan;
        let interval = params.difficulty_adjustment_interval();

        // At height interval-1 (i.e. 2015), the next block (2016) triggers retarget.
        let last_height = (interval - 1) as i32;
        let last_bits = 0x1d00ffff;
        let first_time = 1_000_000u32;
        let last_time = first_time + target_timespan as u32; // exact timespan
        let block_time = last_time + 600;

        let result = get_next_work_required(
            last_height,
            last_bits,
            last_time,
            first_time,
            block_time,
            last_bits,
            &params,
        );
        // Exact timespan => no change.
        assert_eq!(result, last_bits);
    }
}
