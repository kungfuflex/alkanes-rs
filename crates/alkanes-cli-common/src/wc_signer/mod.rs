//! New-protocol WalletConnect signer for SUBFROST mobile (vc=419+).
//!
//! Replaces the old `subfrost-wc` flow (vendor crate) — that protocol
//! used a separate wc-relay HTTP+WSS service; the new flow goes through
//! frtun-pair's `/v1/pair` rendezvous on subfrost-wallet-api, with one
//! shared protocol between the SUBFROST extension, the alkanes-cli, and
//! the `@alkanes/ts-sdk` walletconnect-cli.
//!
//! ## Module split (post-tick-#618 carve)
//!
//! The codec + signer state machine — `wire`, `crypto`, `pairing`,
//! `pair_wake`, `storage` (trait), `transport` (trait), `signer`
//! (driver) — now lives in the sibling crate `wc-signer-core`. This
//! module re-exports the core surface (so existing
//! `alkanes_cli_common::wc_signer::*` import paths keep compiling) and
//! adds the **native-only impls** on top:
//!
//!   * `storage::NativeFileStorage` — file-backed
//!     `~/.alkanes/wc-session.json` (mode 0600)
//!   * `transport::NativeTransport` — tlsfetch-ws + frtun-pair
//!     handshake + an in-tree tokio + tokio-rustls helper
//!     (`http_async`) for the `/v1/pair-wake` POST. Keeps the
//!     wc-signer-native subgraph off reqwest.
//!
//! Wasm consumers (subfrost-wallet-web-sys) depend on `wc-signer-core`
//! directly and supply their own `WasmTransport` / `WasmStorage`. See
//! `~/subfrost-mobile/crates/subfrost-wallet-web-sys/src/wc_signer.rs`.
//!
//! ## Feature gates
//!
//! * `wc-signer` — pulls in `wc-signer-core` (codec-only, wasm-ok).
//! * `wc-signer-native` — adds the native transport + file storage.
//! * `icmp` — typed-frame ping/pong probe in `send_request`; mirrored
//!   on frtun-pair via `frtun-pair/icmp` + `wc-signer-core/icmp`.

pub mod storage;
pub mod transport;

// Shared async H1/H1S helper used by the native `pair_wake` POST so we
// don't pull reqwest into the wc-signer-native subgraph. Engine-agnostic
// — uses only the public rustls 0.23 API. Was an inline module
// (src/wc_signer/http_async.rs) pre-2026-06-10; lifted into the
// vendored `frtun-http-async` crate so the three byte-identical inline
// copies (here, frtun-push-proxy, frtun-pair-bridge) collapse to one.
// Aliased back as `http_async` so existing import paths (e.g.
// `crate::wc_signer::http_async::send_async`) keep resolving.
#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub(crate) use frtun_http_async as http_async;

// Re-export the codec surface from wc-signer-core. Native consumers
// continue to import everything off `alkanes_cli_common::wc_signer::*`
// just like before the carve.
pub use wc_signer_core::{crypto, pair_wake, pairing, signer, wire};
pub use wc_signer_core::{
    PairInit, PersistedSession, Plaintext, SessionStorage, StorageError, TransportError,
    WalletConnectSigner, WalletPairStream, WalletTransport, WcError, WireEnvelope,
};
// Also re-export the `storage::*` / `transport::*` SUB-modules from
// core under the same path, so call sites like
// `alkanes_cli_common::wc_signer::transport::WalletTransport` keep
// resolving. The native impls (NativeFileStorage / NativeTransport)
// live in this crate's own `storage` / `transport` modules above and
// override the re-export at name-resolution time.
pub use wc_signer_core::storage as core_storage;
pub use wc_signer_core::transport as core_transport;

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use storage::NativeFileStorage;
#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use transport::NativeTransport;
