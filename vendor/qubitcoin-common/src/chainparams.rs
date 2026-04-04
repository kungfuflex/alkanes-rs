//! Full chain parameters including consensus params and network specifics.
//!
//! Maps to: `src/kernel/chainparams.h` and `src/kernel/chainparams.cpp` in Bitcoin Core.
//!
//! Provides:
//! - `Network`: Enum identifying mainnet, testnet, regtest, or signet.
//! - `ChainParams`: Full chain parameters for a given network, including
//!   consensus params, default port, address prefixes, and genesis block hash.

use qubitcoin_consensus::block::{Block, BlockHeader};
use qubitcoin_consensus::transaction::{TxIn, TxOut, Transaction};
use qubitcoin_consensus::ConsensusParams;
use qubitcoin_primitives::{Amount, BlockHash, Uint256};
use qubitcoin_script::Script;
use std::sync::Arc;

/// Network type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Bitcoin mainnet (production network).
    Mainnet,
    /// Bitcoin testnet3 (legacy test network).
    Testnet,
    /// Bitcoin testnet4 (newer test network, May 2024).
    Testnet4,
    /// Regression test network (local, instant mining).
    Regtest,
    /// Signet (signature-based test network with centralized block production).
    Signet,
}

/// Full chain parameters including consensus params and network specifics.
///
/// Port of Bitcoin Core's `CChainParams`.
#[derive(Clone)]
pub struct ChainParams {
    /// Which network these parameters are for.
    pub network: Network,

    /// Consensus parameters (PoW limits, activation heights, etc.).
    pub consensus: ConsensusParams,

    /// Default p2p port.
    pub default_port: u16,

    /// Hash of the genesis block.
    pub genesis_block_hash: BlockHash,

    /// DNS seed hostnames for initial peer discovery.
    pub dns_seeds: Vec<String>,

    /// Base58 version byte for pay-to-pubkey-hash addresses.
    pub base58_prefix_pubkey_hash: [u8; 1],

    /// Base58 version byte for pay-to-script-hash addresses.
    pub base58_prefix_script_hash: [u8; 1],

    /// Base58 version byte for WIF-encoded private keys.
    pub base58_prefix_secret_key: [u8; 1],

    /// Bech32 human-readable part (e.g. "bc" for mainnet).
    pub bech32_hrp: String,

    /// BIP44 coin type (0 for mainnet, 1 for all test networks).
    pub bip44_coin_type: u32,

    /// Whether this is a test network.
    pub is_test_chain: bool,

    /// Minimum chain work for the chain to be considered valid.
    pub minimum_chain_work: Uint256,

    /// Assumed-valid block hash (skip full script verification before this).
    pub assumed_valid_block: BlockHash,
}

impl ChainParams {
    /// Parameters for the Bitcoin mainnet.
    pub fn mainnet() -> Self {
        ChainParams {
            network: Network::Mainnet,
            consensus: ConsensusParams::mainnet(),
            default_port: 8333,
            genesis_block_hash: BlockHash::from_hex(
                "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
            )
            .expect("valid mainnet genesis hash"),
            dns_seeds: vec![
                "seed.bitcoin.sipa.be".into(),
                "dnsseed.bluematt.me".into(),
                "seed.bitcoin.jonasschnelli.ch".into(),
                "seed.btc.petertodd.net".into(),
                "seed.bitcoin.sprovoost.nl".into(),
                "dnsseed.emzy.de".into(),
                "seed.bitcoin.wiz.biz".into(),
                "seed.mainnet.achownodes.xyz".into(),
            ],
            base58_prefix_pubkey_hash: [0x00],
            base58_prefix_script_hash: [0x05],
            base58_prefix_secret_key: [0x80],
            bech32_hrp: "bc".into(),
            bip44_coin_type: 0,
            is_test_chain: false,
            minimum_chain_work: ConsensusParams::mainnet().minimum_chain_work,
            // Block 900,000 — skip script verification for all ancestor blocks.
            assumed_valid_block: BlockHash::from_hex(
                "000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a",
            )
            .expect("valid assume-valid hash"),
        }
    }

    /// Parameters for Bitcoin testnet3.
    pub fn testnet() -> Self {
        ChainParams {
            network: Network::Testnet,
            consensus: ConsensusParams::testnet(),
            default_port: 18333,
            genesis_block_hash: BlockHash::from_hex(
                "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943",
            )
            .expect("valid testnet genesis hash"),
            dns_seeds: vec![
                "testnet-seed.bitcoin.jonasschnelli.ch".into(),
                "seed.tbtc.petertodd.net".into(),
                "seed.testnet.bitcoin.sprovoost.nl".into(),
                "testnet-seed.bluematt.me".into(),
                "seed.testnet.achownodes.xyz".into(),
            ],
            base58_prefix_pubkey_hash: [0x6f],
            base58_prefix_script_hash: [0xc4],
            base58_prefix_secret_key: [0xef],
            bech32_hrp: "tb".into(),
            bip44_coin_type: 1,
            is_test_chain: true,
            minimum_chain_work: Uint256::ZERO,
            assumed_valid_block: BlockHash::ZERO,
        }
    }

    /// Parameters for the regtest (regression test) network.
    pub fn regtest() -> Self {
        ChainParams {
            network: Network::Regtest,
            consensus: ConsensusParams::regtest(),
            default_port: 18444,
            genesis_block_hash: BlockHash::from_hex(
                "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
            )
            .expect("valid regtest genesis hash"),
            dns_seeds: vec![],
            base58_prefix_pubkey_hash: [0x6f],
            base58_prefix_script_hash: [0xc4],
            base58_prefix_secret_key: [0xef],
            bech32_hrp: "bcrt".into(),
            bip44_coin_type: 1,
            is_test_chain: true,
            minimum_chain_work: Uint256::ZERO,
            assumed_valid_block: BlockHash::ZERO,
        }
    }

    /// Parameters for the signet network.
    pub fn signet() -> Self {
        ChainParams {
            network: Network::Signet,
            consensus: ConsensusParams::signet(),
            default_port: 38333,
            genesis_block_hash: BlockHash::from_hex(
                "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6",
            )
            .expect("valid signet genesis hash"),
            dns_seeds: vec![
                "seed.signet.bitcoin.sprovoost.nl".into(),
                "seed.signet.achownodes.xyz".into(),
            ],
            base58_prefix_pubkey_hash: [0x6f],
            base58_prefix_script_hash: [0xc4],
            base58_prefix_secret_key: [0xef],
            bech32_hrp: "tb".into(),
            bip44_coin_type: 1,
            is_test_chain: true,
            minimum_chain_work: Uint256::ZERO,
            assumed_valid_block: BlockHash::ZERO,
        }
    }

    /// Parameters for Bitcoin testnet4.
    pub fn testnet4() -> Self {
        ChainParams {
            network: Network::Testnet4,
            consensus: ConsensusParams::testnet4(),
            default_port: 48333,
            genesis_block_hash: BlockHash::from_hex(
                "00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043",
            )
            .expect("valid testnet4 genesis hash"),
            dns_seeds: vec![
                "seed.testnet4.bitcoin.sprovoost.nl.".into(),
                "seed.testnet4.wiz.biz.".into(),
            ],
            base58_prefix_pubkey_hash: [0x6f],
            base58_prefix_script_hash: [0xc4],
            base58_prefix_secret_key: [0xef],
            bech32_hrp: "tb".into(),
            bip44_coin_type: 1,
            is_test_chain: true,
            minimum_chain_work: Uint256::ZERO,
            assumed_valid_block: BlockHash::ZERO,
        }
    }

    /// Create the genesis block for this network.
    ///
    /// Port of Bitcoin Core's `CreateGenesisBlock()` in `src/kernel/chainparams.cpp`.
    /// Most networks share the standard Satoshi coinbase transaction, but testnet4
    /// uses a different message and output script.
    pub fn create_genesis_block(&self) -> Block {
        // Build the coinbase scriptsig and output script.
        //
        // Bitcoin Core constructs the scriptsig as:
        //   CScript() << 486604799 << CScriptNum(4) << message_bytes
        // which encodes to: 04 ffff001d 01 04 <pushdata_for_message> <message>
        let (script_sig, genesis_output_script) = match self.network {
            Network::Testnet4 => {
                // Testnet4 message: "03/May/2024 000000000000000000001ebd58c244970b3aa9d783bb001011fbe8ea8e98e00e"
                // The message is 76 bytes, so it requires OP_PUSHDATA1 (0x4c) + length byte.
                // scriptsig: 04 ffff001d 01 04 4c 4c <76 bytes of message>
                let script_sig = Script::from_bytes(vec![
                    0x04, 0xff, 0xff, 0x00, 0x1d, 0x01, 0x04, 0x4c,
                    0x4c, // OP_PUSHDATA1, length=76
                    0x30, 0x33, 0x2f, 0x4d, 0x61, 0x79, 0x2f, 0x32,
                    0x30, 0x32, 0x34, 0x20, 0x30, 0x30, 0x30, 0x30,
                    0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
                    0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
                    0x31, 0x65, 0x62, 0x64, 0x35, 0x38, 0x63, 0x32,
                    0x34, 0x34, 0x39, 0x37, 0x30, 0x62, 0x33, 0x61,
                    0x61, 0x39, 0x64, 0x37, 0x38, 0x33, 0x62, 0x62,
                    0x30, 0x30, 0x31, 0x30, 0x31, 0x31, 0x66, 0x62,
                    0x65, 0x38, 0x65, 0x61, 0x38, 0x65, 0x39, 0x38,
                    0x65, 0x30, 0x30, 0x65,
                ]);
                // Output script: push 33 zero bytes (compressed pubkey) + OP_CHECKSIG
                let output_script = Script::from_bytes(vec![
                    0x21, // push 33 bytes
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, // 33 zero bytes
                    0xac, // OP_CHECKSIG
                ]);
                (script_sig, output_script)
            }
            _ => {
                // Standard Satoshi genesis coinbase (mainnet, testnet3, regtest, signet).
                // Message: "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks"
                // (69 bytes, pushed with opcode 0x45)
                // scriptsig: 04ffff001d0104455468652054696d65732030332f4a616e2f323030392043
                //            68616e63656c6c6f72206f6e206272696e6b206f66207365636f6e642062
                //            61696c6f757420666f722062616e6b73
                let script_sig = Script::from_bytes(vec![
                    0x04, 0xff, 0xff, 0x00, 0x1d, 0x01, 0x04, 0x45,
                    0x54, 0x68, 0x65, 0x20, 0x54, 0x69, 0x6d, 0x65,
                    0x73, 0x20, 0x30, 0x33, 0x2f, 0x4a, 0x61, 0x6e,
                    0x2f, 0x32, 0x30, 0x30, 0x39, 0x20, 0x43, 0x68,
                    0x61, 0x6e, 0x63, 0x65, 0x6c, 0x6c, 0x6f, 0x72,
                    0x20, 0x6f, 0x6e, 0x20, 0x62, 0x72, 0x69, 0x6e,
                    0x6b, 0x20, 0x6f, 0x66, 0x20, 0x73, 0x65, 0x63,
                    0x6f, 0x6e, 0x64, 0x20, 0x62, 0x61, 0x69, 0x6c,
                    0x6f, 0x75, 0x74, 0x20, 0x66, 0x6f, 0x72, 0x20,
                    0x62, 0x61, 0x6e, 0x6b, 0x73,
                ]);
                // Output script: push 65-byte uncompressed pubkey + OP_CHECKSIG
                let output_script = Script::from_bytes(vec![
                    0x41, // push 65 bytes
                    0x04, 0x67, 0x8a, 0xfd, 0xb0, 0xfe, 0x55, 0x48,
                    0x27, 0x19, 0x67, 0xf1, 0xa6, 0x71, 0x30, 0xb7,
                    0x10, 0x5c, 0xd6, 0xa8, 0x28, 0xe0, 0x39, 0x09,
                    0xa6, 0x79, 0x62, 0xe0, 0xea, 0x1f, 0x61, 0xde,
                    0xb6, 0x49, 0xf6, 0xbc, 0x3f, 0x4c, 0xef, 0x38,
                    0xc4, 0xf3, 0x55, 0x04, 0xe5, 0x1e, 0xc1, 0x12,
                    0xde, 0x5c, 0x38, 0x4d, 0xf7, 0xba, 0x0b, 0x8d,
                    0x57, 0x8a, 0x4c, 0x70, 0x2b, 0x6b, 0xf1, 0x1d,
                    0x5f, // end of 65-byte pubkey
                    0xac, // OP_CHECKSIG
                ]);
                (script_sig, output_script)
            }
        };

        let txin = TxIn::coinbase(script_sig);
        let txout = TxOut::new(Amount::from_sat(50 * 100_000_000), genesis_output_script);
        let genesis_tx = Transaction::new(1, vec![txin], vec![txout], 0);

        // For a single-transaction block, the merkle root is simply the txid.
        let merkle_root = genesis_tx.txid().into_uint256();

        // Network-specific header parameters.
        let (time, bits, nonce) = match self.network {
            Network::Mainnet => (1231006505u32, 0x1d00ffffu32, 2083236893u32),
            Network::Testnet => (1296688602, 0x1d00ffff, 414098458),
            Network::Testnet4 => (1714777860, 0x1d00ffff, 393743547),
            Network::Regtest => (1296688602, 0x207fffff, 2),
            Network::Signet => (1598918400, 0x1e0377ae, 52613770),
        };

        let header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::ZERO,
            merkle_root,
            time,
            bits,
            nonce,
        };

        Block {
            header,
            vtx: vec![Arc::new(genesis_tx)],
        }
    }

    /// Get chain parameters for a given network.
    pub fn for_network(network: Network) -> Self {
        match network {
            Network::Mainnet => Self::mainnet(),
            Network::Testnet => Self::testnet(),
            Network::Testnet4 => Self::testnet4(),
            Network::Regtest => Self::regtest(),
            Network::Signet => Self::signet(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Genesis hash tests --

    #[test]
    fn test_mainnet_genesis_hash() {
        let params = ChainParams::mainnet();
        assert_eq!(
            params.genesis_block_hash.to_hex(),
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
        );
    }

    #[test]
    fn test_testnet_genesis_hash() {
        let params = ChainParams::testnet();
        assert_eq!(
            params.genesis_block_hash.to_hex(),
            "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943"
        );
    }

    #[test]
    fn test_regtest_genesis_hash() {
        let params = ChainParams::regtest();
        assert_eq!(
            params.genesis_block_hash.to_hex(),
            "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
        );
    }

    #[test]
    fn test_signet_genesis_hash() {
        let params = ChainParams::signet();
        assert_eq!(
            params.genesis_block_hash.to_hex(),
            "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6"
        );
    }

    // -- Default port tests --

    #[test]
    fn test_mainnet_default_port() {
        let params = ChainParams::mainnet();
        assert_eq!(params.default_port, 8333);
    }

    #[test]
    fn test_testnet_default_port() {
        let params = ChainParams::testnet();
        assert_eq!(params.default_port, 18333);
    }

    #[test]
    fn test_regtest_default_port() {
        let params = ChainParams::regtest();
        assert_eq!(params.default_port, 18444);
    }

    #[test]
    fn test_signet_default_port() {
        let params = ChainParams::signet();
        assert_eq!(params.default_port, 38333);
    }

    // -- Address prefix tests --

    #[test]
    fn test_mainnet_address_prefixes() {
        let params = ChainParams::mainnet();
        assert_eq!(params.base58_prefix_pubkey_hash, [0x00]);
        assert_eq!(params.base58_prefix_script_hash, [0x05]);
        assert_eq!(params.base58_prefix_secret_key, [0x80]);
        assert_eq!(params.bech32_hrp, "bc");
        assert_eq!(params.bip44_coin_type, 0);
        assert!(!params.is_test_chain);
    }

    #[test]
    fn test_testnet_address_prefixes() {
        let params = ChainParams::testnet();
        assert_eq!(params.base58_prefix_pubkey_hash, [0x6f]);
        assert_eq!(params.base58_prefix_script_hash, [0xc4]);
        assert_eq!(params.base58_prefix_secret_key, [0xef]);
        assert_eq!(params.bech32_hrp, "tb");
        assert_eq!(params.bip44_coin_type, 1);
        assert!(params.is_test_chain);
    }

    #[test]
    fn test_regtest_address_prefixes() {
        let params = ChainParams::regtest();
        assert_eq!(params.base58_prefix_pubkey_hash, [0x6f]);
        assert_eq!(params.base58_prefix_script_hash, [0xc4]);
        assert_eq!(params.base58_prefix_secret_key, [0xef]);
        assert_eq!(params.bech32_hrp, "bcrt");
        assert_eq!(params.bip44_coin_type, 1);
        assert!(params.is_test_chain);
    }

    #[test]
    fn test_signet_address_prefixes() {
        let params = ChainParams::signet();
        assert_eq!(params.base58_prefix_pubkey_hash, [0x6f]);
        assert_eq!(params.base58_prefix_script_hash, [0xc4]);
        assert_eq!(params.base58_prefix_secret_key, [0xef]);
        assert_eq!(params.bech32_hrp, "tb");
        assert_eq!(params.bip44_coin_type, 1);
        assert!(params.is_test_chain);
    }

    // -- for_network dispatch test --

    #[test]
    fn test_for_network_dispatch() {
        let networks = [
            Network::Mainnet,
            Network::Testnet,
            Network::Testnet4,
            Network::Regtest,
            Network::Signet,
        ];
        let expected_ports = [8333u16, 18333, 48333, 18444, 38333];

        for (network, expected_port) in networks.iter().zip(expected_ports.iter()) {
            let params = ChainParams::for_network(*network);
            assert_eq!(params.network, *network);
            assert_eq!(params.default_port, *expected_port);
        }
    }

    #[test]
    fn test_mainnet_consensus_params() {
        let params = ChainParams::mainnet();
        assert_eq!(params.consensus.subsidy_halving_interval, 210_000);
        assert_eq!(params.consensus.pow_target_spacing, 600);
        assert!(!params.consensus.pow_allow_min_difficulty_blocks);
        assert!(!params.consensus.pow_no_retargeting);
    }

    #[test]
    fn test_regtest_consensus_params() {
        let params = ChainParams::regtest();
        assert_eq!(params.consensus.subsidy_halving_interval, 150);
        assert!(params.consensus.pow_allow_min_difficulty_blocks);
        assert!(params.consensus.pow_no_retargeting);
    }

    #[test]
    fn test_mainnet_has_dns_seeds() {
        let params = ChainParams::mainnet();
        assert!(!params.dns_seeds.is_empty());
    }

    #[test]
    fn test_regtest_has_no_dns_seeds() {
        let params = ChainParams::regtest();
        assert!(params.dns_seeds.is_empty());
    }

    // -- create_genesis_block tests --

    #[test]
    fn test_mainnet_genesis_block_hash() {
        let params = ChainParams::mainnet();
        let genesis = params.create_genesis_block();
        assert_eq!(
            genesis.block_hash().to_hex(),
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
        );
    }

    #[test]
    fn test_testnet_genesis_block_hash() {
        let params = ChainParams::testnet();
        let genesis = params.create_genesis_block();
        assert_eq!(
            genesis.block_hash().to_hex(),
            "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943"
        );
    }

    #[test]
    fn test_testnet4_genesis_block_hash() {
        let params = ChainParams::testnet4();
        let genesis = params.create_genesis_block();
        assert_eq!(
            genesis.block_hash().to_hex(),
            "00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043"
        );
    }

    #[test]
    fn test_regtest_genesis_block_hash() {
        let params = ChainParams::regtest();
        let genesis = params.create_genesis_block();
        assert_eq!(
            genesis.block_hash().to_hex(),
            "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
        );
    }

    #[test]
    fn test_signet_genesis_block_hash() {
        let params = ChainParams::signet();
        let genesis = params.create_genesis_block();
        assert_eq!(
            genesis.block_hash().to_hex(),
            "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6"
        );
    }

    #[test]
    fn test_genesis_block_is_coinbase() {
        let params = ChainParams::mainnet();
        let genesis = params.create_genesis_block();
        assert_eq!(genesis.vtx.len(), 1);
        assert!(genesis.vtx[0].is_coinbase());
    }

    #[test]
    fn test_genesis_block_matches_stored_hash() {
        // Verify create_genesis_block().block_hash() matches genesis_block_hash
        // for every network.
        for network in &[
            Network::Mainnet,
            Network::Testnet,
            Network::Testnet4,
            Network::Regtest,
            Network::Signet,
        ] {
            let params = ChainParams::for_network(*network);
            let genesis = params.create_genesis_block();
            assert_eq!(
                genesis.block_hash(),
                params.genesis_block_hash,
                "genesis block hash mismatch for {:?}",
                network
            );
        }
    }
}
