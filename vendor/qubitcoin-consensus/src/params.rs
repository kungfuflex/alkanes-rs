//! Consensus parameters for different networks.
//! Maps to: src/consensus/params.h, src/kernel/chainparams.cpp

use qubitcoin_primitives::{BlockHash, Uint256};
use std::collections::HashMap;

/// Consensus parameters for a network.
///
/// Port of Bitcoin Core's Consensus::Params.
#[derive(Clone, Debug)]
pub struct ConsensusParams {
    /// Block height at which BIP34 becomes active (block height in coinbase).
    pub bip34_height: i32,
    /// Block height at which BIP65 becomes active (CHECKLOCKTIMEVERIFY).
    pub bip65_height: i32,
    /// Block height at which BIP66 becomes active (strict DER signatures).
    pub bip66_height: i32,
    /// Block height at which CSV (BIP68, BIP112, BIP113) becomes active.
    pub csv_height: i32,
    /// Block height at which segwit (BIP141, BIP143, BIP147) becomes active.
    pub segwit_height: i32,
    /// Block height at which Taproot (BIP341, BIP342) becomes active.
    /// -1 means never active. Bitcoin Core uses version bits for this on
    /// mainnet, but we simplify to a height for the same effect.
    pub taproot_height: i32,

    /// Map of block hashes to script verification flag overrides.
    /// Used for the two historical blocks that violated P2SH/Taproot rules.
    /// Maps to: `Consensus::Params::script_flag_exceptions` in Bitcoin Core.
    pub script_flag_exceptions: HashMap<BlockHash, u32>,

    /// Minimum blocks including miner confirmation of the total of 2016 blocks in a retargeting period.
    pub rule_change_activation_threshold: u32,
    /// Number of blocks in a retargeting period.
    pub miner_confirmation_window: u32,

    /// Proof-of-work upper bound. No target may exceed this value.
    pub pow_limit: Uint256,
    /// Target timespan for difficulty adjustment, in seconds (e.g., 2 weeks for mainnet).
    pub pow_target_timespan: i64,
    /// Target time between blocks, in seconds (e.g., 600 for mainnet = 10 minutes).
    pub pow_target_spacing: i64,
    /// Whether to allow minimum-difficulty blocks (testnet rule).
    pub pow_allow_min_difficulty_blocks: bool,
    /// Whether to disable difficulty retargeting entirely (regtest rule).
    pub pow_no_retargeting: bool,

    /// The best chain should have at least this much work.
    pub minimum_chain_work: Uint256,

    /// Block hash that must be in the chain.
    pub default_assume_valid: BlockHash,

    /// Subsidy halving interval (blocks).
    pub subsidy_halving_interval: i32,
}

impl ConsensusParams {
    /// Mainnet consensus parameters.
    pub fn mainnet() -> Self {
        // Historical blocks that violated script rules.
        // BIP16 exception + Taproot exception (P2SH+WITNESS only, no TAPROOT).
        let mut exceptions = HashMap::new();
        // Block that violated BIP16 P2SH rules:
        if let Some(h) =
            BlockHash::from_hex("00000000000002dc756eebf4f49723ed8d30cc28a5f108eb94b1ba88ac4f9c22")
        {
            exceptions.insert(h, 0u32); // SCRIPT_VERIFY_NONE
        }
        // Block that violated Taproot rules (only P2SH+WITNESS):
        if let Some(h) =
            BlockHash::from_hex("0000000000000000000f14c35b2d841e986ab5441de8c585d5ffe55ea1e395ad")
        {
            exceptions.insert(h, 0x801u32); // SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS
        }

        ConsensusParams {
            bip34_height: 227931,
            bip65_height: 388381,
            bip66_height: 363725,
            csv_height: 419328,
            segwit_height: 481824,
            taproot_height: 709632,
            script_flag_exceptions: exceptions,
            rule_change_activation_threshold: 1916,
            miner_confirmation_window: 2016,
            pow_limit: Uint256::from_hex(
                "00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .unwrap(),
            pow_target_timespan: 14 * 24 * 60 * 60, // 2 weeks
            pow_target_spacing: 10 * 60,            // 10 minutes
            pow_allow_min_difficulty_blocks: false,
            pow_no_retargeting: false,
            minimum_chain_work: Uint256::from_hex(
                "0000000000000000000000000000000000000000dee8e2a309ad8a9820433c68",
            )
            .unwrap(),
            default_assume_valid: BlockHash::from_hex(
                "00000000000000000000611fd22f2df7c8fbd0688745c3a6c3bb5109cc2a12cb",
            )
            .unwrap_or(BlockHash::ZERO),
            subsidy_halving_interval: 210_000,
        }
    }

    /// Testnet3 consensus parameters.
    pub fn testnet() -> Self {
        // BIP16 exception block on testnet3.
        let mut exceptions = HashMap::new();
        if let Some(h) =
            BlockHash::from_hex("00000000dd30457c001f4095d208cc1296b0eed002427aa599874af7a432b105")
        {
            exceptions.insert(h, 0u32); // SCRIPT_VERIFY_NONE
        }

        ConsensusParams {
            bip34_height: 21111,
            bip65_height: 581885,
            bip66_height: 330776,
            csv_height: 770112,
            segwit_height: 834624,
            taproot_height: 0, // always active on testnet3
            script_flag_exceptions: exceptions,
            rule_change_activation_threshold: 1512,
            miner_confirmation_window: 2016,
            pow_limit: Uint256::from_hex(
                "00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .unwrap(),
            pow_target_timespan: 14 * 24 * 60 * 60,
            pow_target_spacing: 10 * 60,
            pow_allow_min_difficulty_blocks: true,
            pow_no_retargeting: false,
            minimum_chain_work: Uint256::from_hex(
                "0000000000000000000000000000000000000000000016dd270dd94fac1d7632",
            )
            .unwrap(),
            default_assume_valid: BlockHash::from_hex(
                "0000000000000065c6c38258e201971a3fdfcc2ceee0dd6e85a6c022d45dee34",
            )
            .unwrap_or(BlockHash::ZERO),
            subsidy_halving_interval: 210_000,
        }
    }

    /// Regtest consensus parameters.
    ///
    /// Matches Bitcoin Core: all BIPs active from height 1 (or 0 for segwit/taproot).
    pub fn regtest() -> Self {
        ConsensusParams {
            bip34_height: 1,
            bip65_height: 1,
            bip66_height: 1,
            csv_height: 1,
            segwit_height: 0,  // always active
            taproot_height: 0, // always active
            script_flag_exceptions: HashMap::new(),
            rule_change_activation_threshold: 108,
            miner_confirmation_window: 144,
            pow_limit: Uint256::from_hex(
                "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .unwrap(),
            pow_target_timespan: 24 * 60 * 60, // 1 day (Bitcoin Core regtest)
            pow_target_spacing: 10 * 60,
            pow_allow_min_difficulty_blocks: true,
            pow_no_retargeting: true,
            minimum_chain_work: Uint256::ZERO,
            default_assume_valid: BlockHash::ZERO,
            subsidy_halving_interval: 150,
        }
    }

    /// Signet consensus parameters.
    ///
    /// Matches Bitcoin Core: all BIPs active from height 1, custom powLimit.
    pub fn signet() -> Self {
        ConsensusParams {
            bip34_height: 1,
            bip65_height: 1,
            bip66_height: 1,
            csv_height: 1,
            segwit_height: 1,
            taproot_height: 0, // always active
            script_flag_exceptions: HashMap::new(),
            rule_change_activation_threshold: 1916,
            miner_confirmation_window: 2016,
            pow_limit: Uint256::from_hex(
                "00000377ae000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            pow_target_timespan: 14 * 24 * 60 * 60,
            pow_target_spacing: 10 * 60,
            pow_allow_min_difficulty_blocks: false,
            pow_no_retargeting: false,
            minimum_chain_work: Uint256::from_hex(
                "0000000000000000000000000000000000000000000000000000067d328e681a",
            )
            .unwrap(),
            default_assume_valid: BlockHash::from_hex(
                "000000128586e26813922680309f04e1de713c7542fee86ed908f56368aefe2e",
            )
            .unwrap_or(BlockHash::ZERO),
            subsidy_halving_interval: 210_000,
        }
    }

    /// Testnet4 consensus parameters.
    ///
    /// Matches Bitcoin Core: all BIPs active from height 1, Taproot always active.
    pub fn testnet4() -> Self {
        ConsensusParams {
            bip34_height: 1,
            bip65_height: 1,
            bip66_height: 1,
            csv_height: 1,
            segwit_height: 1,
            taproot_height: 0, // always active
            script_flag_exceptions: HashMap::new(),
            rule_change_activation_threshold: 1512,
            miner_confirmation_window: 2016,
            pow_limit: Uint256::from_hex(
                "00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .unwrap(),
            pow_target_timespan: 14 * 24 * 60 * 60,
            pow_target_spacing: 10 * 60,
            pow_allow_min_difficulty_blocks: true,
            pow_no_retargeting: false,
            minimum_chain_work: Uint256::ZERO,
            default_assume_valid: BlockHash::ZERO,
            subsidy_halving_interval: 210_000,
        }
    }

    /// Get the difficulty adjustment interval (how many blocks between retargets).
    pub fn difficulty_adjustment_interval(&self) -> i64 {
        self.pow_target_timespan / self.pow_target_spacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mainnet_params() {
        let params = ConsensusParams::mainnet();
        assert_eq!(params.subsidy_halving_interval, 210_000);
        assert_eq!(params.difficulty_adjustment_interval(), 2016);
        assert_eq!(params.pow_target_spacing, 600);
    }

    #[test]
    fn test_regtest_params() {
        let params = ConsensusParams::regtest();
        assert_eq!(params.subsidy_halving_interval, 150);
        assert!(params.pow_no_retargeting);
        assert!(params.pow_allow_min_difficulty_blocks);
        assert_eq!(params.segwit_height, 0);
    }
}
