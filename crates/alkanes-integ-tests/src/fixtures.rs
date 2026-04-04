//! WASM binary fixtures loaded via include_bytes!

/// alkanes.wasm — primary indexer (v2.1.6 regtest)
pub const ALKANES_WASM: &[u8] = include_bytes!("../test_data/alkanes.wasm");

/// esplorashrew.wasm — Esplora-compatible secondary indexer
pub const ESPLORA_WASM: &[u8] = include_bytes!("../test_data/esplorashrew.wasm");

/// shrew_ord.wasm — Ordinals/inscriptions secondary indexer
pub const ORD_WASM: &[u8] = include_bytes!("../test_data/shrew_ord.wasm");

/// alkanes_std_test — test contract with opcode 30 (arb_mint) and opcode 5 (forward)
pub const TEST_CONTRACT: &[u8] = include_bytes!("../test_data/alkanes_std_test.wasm");

/// alkanes_std_auth_token — auth token factory
pub const AUTH_TOKEN: &[u8] = include_bytes!("../test_data/alkanes_std_auth_token.wasm");

/// alkanes_std_beacon_proxy — beacon proxy contract
pub const BEACON_PROXY: &[u8] = include_bytes!("../test_data/alkanes_std_beacon_proxy.wasm");

/// alkanes_std_upgradeable — upgradeable proxy contract
pub const UPGRADEABLE: &[u8] = include_bytes!("../test_data/alkanes_std_upgradeable.wasm");

/// alkanes_std_upgradeable_beacon — upgradeable beacon contract
pub const UPGRADEABLE_BEACON: &[u8] =
    include_bytes!("../test_data/alkanes_std_upgradeable_beacon.wasm");

/// factory.wasm — AMM factory logic
pub const FACTORY: &[u8] = include_bytes!("../test_data/factory.wasm");

/// pool.wasm — AMM pool logic
pub const POOL: &[u8] = include_bytes!("../test_data/pool.wasm");

/// alkanes_std_owned_token — owned token contract
pub const OWNED_TOKEN: &[u8] = include_bytes!("../test_data/alkanes_std_owned_token.wasm");

/// alkanes_std_test_2 — second test contract (for upgrade tests)
pub const TEST_CONTRACT_2: &[u8] = include_bytes!("../test_data/alkanes_std_test_2.wasm");

/// Genesis alkane (regtest)
pub const GENESIS_ALKANE: &[u8] =
    include_bytes!("../test_data/alkanes_std_genesis_alkane_regtest.wasm");

/// Upgraded genesis alkane (regtest)
pub const GENESIS_ALKANE_UPGRADED: &[u8] =
    include_bytes!("../test_data/alkanes_std_genesis_alkane_upgraded_regtest.wasm");

/// Merkle distributor (regtest)
pub const MERKLE_DISTRIBUTOR: &[u8] =
    include_bytes!("../test_data/alkanes_std_merkle_distributor_regtest.wasm");

/// fr_btc — fractionalized BTC contract
pub const FR_BTC: &[u8] = include_bytes!("../test_data/fr_btc.wasm");

/// frusd_auth_token — frUSD-specific auth token (opcode 0 = initialize, mints 1 to deployer)
pub const FRUSD_AUTH_TOKEN: &[u8] = include_bytes!("../test_data/frusd_auth_token.wasm");

/// frusd_token — frUSD stablecoin token (opcode 1 = mint w/ auth, opcode 5 = burn+bridge)
pub const FRUSD_TOKEN: &[u8] = include_bytes!("../test_data/frusd_token.wasm");
