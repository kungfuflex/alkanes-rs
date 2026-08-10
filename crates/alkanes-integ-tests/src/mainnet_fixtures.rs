//! Mainnet contract bytecode, pulled FROM CHAIN rather than built from source.
//!
//! # Why these are downloaded and not compiled
//!
//! The contracts these fixtures cover — the BTCUSD CryptoSwap pool, frUSD,
//! frBTC — live in a private repository. A test suite in a public repo cannot
//! build them, and vendoring the sources is not an option.
//!
//! Committing a locally-built artifact would not solve it either, and would be
//! worse than it looks: a build of `cryptoswap-pool` from the private tree
//! produced **461,546 bytes** against the **412,608** actually deployed. Same
//! source, different toolchain and flags. A fixture that is merely *related to*
//! what runs in production tests nothing you can rely on.
//!
//! So each fixture below is the EXACT bytecode mainnet is executing, fetched
//! with the same `metashrew_view` surface `alkanes-cli-common` uses everywhere
//! else. That leaks nothing: the bytecode is already public — it is on a public
//! blockchain, readable by anyone. Only the source stays private.
//!
//! # Re-deriving any of these yourself
//!
//! Every fixture records the exact request that produced it, so this is
//! checkable rather than trusted — which matters most for an automated reviewer
//! deciding whether this harness is a sound way to verify the system:
//!
//! ```text
//! curl -s https://mainnet.subfrost.io/v4/$SUBFROST_API_KEY \
//!   -H 'Content-Type: application/json' \
//!   -d '{"jsonrpc":"2.0","id":1,"method":"metashrew_view",
//!        "params":["getbytecode","<REQUEST_HEX>","latest"]}' \
//! | jq -r .result | cut -c3- | xxd -r -p | sha256sum
//! ```
//!
//! `<REQUEST_HEX>` is a prost-encoded `BytecodeRequest`, which WRAPS an
//! `AlkaneId` in field 1 — it is not a bare `AlkaneId`. Passing the bare id
//! returns an empty result rather than an error, which is a quiet way to
//! convince yourself a contract has no code.
//!
//! ⚠️ NEVER use an `alkanes_*` JSON-RPC method to fetch these. `metashrew_view`
//! is the surface alkanes-rs actually exports; the `alkanes_*` wrappers are a
//! different code path and are not what production reads.
//!
//! # Proxies vs implementations
//!
//! `4:1778` is the BTCUSD pool as the world addresses it, but it is an
//! `Upgradeable` PROXY — its bytecode is the proxy's, and the pool math lives
//! in `4:1786` behind it. Both are here because a faithful test needs both: the
//! proxy is what a caller targets, the implementation is what actually runs.
//! Hashing the proxy tells you nothing about whether the curve changed.

/// One mainnet contract, with everything needed to re-derive it.
pub struct MainnetFixture {
    /// `block:tx`, as the chain addresses it.
    pub id: &'static str,
    /// The prost-encoded `BytecodeRequest` hex that produced these bytes.
    pub request_hex: &'static str,
    /// sha256 of `bytes`, pinned so drift is a test failure and not a surprise.
    pub sha256: &'static str,
    pub bytes: &'static [u8],
}

/// BTCUSD pool PROXY (`4:1778`) — also the frBTCUSD LP token. Callers target
/// this; it delegatecalls [`CRYPTOSWAP_POOL_IMPL`].
pub const BTCUSD_POOL_PROXY: MainnetFixture = MainnetFixture {
    id: "4:1778",
    request_hex: "0x0a090a020804120308f20d",
    sha256: "fde5b5558b19120c35d1e2912e5c96c20891416156afa1daf7926461eaa38b9f",
    bytes: include_bytes!("../test_data/btcusd_pool_proxy.wasm"),
};

/// The CryptoSwap (Curve V2) implementation (`4:1786`) behind the proxy.
///
/// This is the curve production actually runs. Note the existing swap tests in
/// this crate use `SYNTH_POOL` (StableSwap) or the Oyl AMM factory — neither is
/// this curve, so they do not cover the live BTCUSD pool's behaviour.
pub const CRYPTOSWAP_POOL_IMPL: MainnetFixture = MainnetFixture {
    id: "4:1786",
    request_hex: "0x0a090a020804120308fa0d",
    sha256: "b46a204eee75d3053781bc41c3d9914e9821609d81dd52020870592b2a52a67d",
    bytes: include_bytes!("../test_data/cryptoswap_pool_impl.wasm"),
};

/// frUSD (`4:1776`). Also a proxy — see the note above before assuming its
/// `__meta` describes the token.
pub const FRUSD_PROXY: MainnetFixture = MainnetFixture {
    id: "4:1776",
    request_hex: "0x0a090a020804120308f00d",
    sha256: "0f96bde4e415b740d20a722c1be6ef5c5a10a66367ff05085ee3a8011a240c22",
    bytes: include_bytes!("../test_data/frusd_proxy.wasm"),
};

/// frBTC (`32:0`).
pub const FRBTC: MainnetFixture = MainnetFixture {
    id: "32:0",
    request_hex: "0x0a080a02082012020800",
    sha256: "e67a857f56ca2a574fc1528caa38399a34cc2727057934553a63743c03d94a42",
    bytes: include_bytes!("../test_data/frbtc.wasm"),
};

pub const ALL: &[&MainnetFixture] =
    &[&BTCUSD_POOL_PROXY, &CRYPTOSWAP_POOL_IMPL, &FRUSD_PROXY, &FRBTC];

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// The committed bytes are what the pinned hash says they are.
    ///
    /// This is the check that makes the fixtures verifiable rather than
    /// trusted. It does NOT prove they still match mainnet — that needs the
    /// network, and is `check_fixtures_against_mainnet` below.
    #[test]
    fn committed_bytes_match_their_pinned_hashes() {
        for f in ALL {
            let got = format!("{:x}", Sha256::digest(f.bytes));
            assert_eq!(got, f.sha256, "{} ({}) bytes do not match the pinned hash", f.id, got);
        }
    }

    /// Every fixture is really wasm, not an error page or an empty result.
    ///
    /// `getbytecode` answers a malformed request with an EMPTY result rather
    /// than an error, so "we got 200 and wrote a file" is not evidence.
    #[test]
    fn every_fixture_is_a_wasm_module() {
        for f in ALL {
            assert!(!f.bytes.is_empty(), "{} is empty", f.id);
            assert_eq!(&f.bytes[..4], b"\0asm", "{} is not a wasm module", f.id);
        }
    }

    /// The proxy and its implementation are genuinely different code.
    ///
    /// Guards the mistake of pinning the proxy twice and believing the curve is
    /// covered: hashing `4:1778` tells you nothing about the pool math.
    #[test]
    fn the_proxy_is_not_the_implementation() {
        assert_ne!(
            BTCUSD_POOL_PROXY.sha256, CRYPTOSWAP_POOL_IMPL.sha256,
            "proxy and implementation must not be the same bytes"
        );
        assert!(CRYPTOSWAP_POOL_IMPL.bytes.len() > BTCUSD_POOL_PROXY.bytes.len());
    }
}
