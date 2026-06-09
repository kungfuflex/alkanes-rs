//! New-protocol WalletConnect signer for SUBFROST mobile (vc=419+).
//!
//! Replaces the old `subfrost-wc` flow (vendor crate) — that protocol
//! used a separate wc-relay HTTP+WSS service; the new flow goes through
//! frtun-pair's `/v1/pair` rendezvous on subfrost-wallet-api, with one
//! shared protocol between the SUBFROST extension, the alkanes-cli, and
//! the `@alkanes/ts-sdk` walletconnect-cli.
//!
//! Module split:
//!   * `wire`      — `Plaintext` tagged-union + `WireEnvelope`
//!                   (camelCase to match the TS sender byte-for-byte).
//!   * `crypto`    — X25519 + HKDF-SHA256 + ChaCha20-Poly1305.
//!   * `pairing`   — `subfrost://wc/...` URI build + parse + code/peer
//!                   mint.
//!   * `pair_wake` — `/v1/pair-wake` HTTP wire shapes.
//!   * `transport` — `WalletTransport` trait + native impl (frtun_pair
//!                   client + reqwest POST).
//!   * `storage`   — `SessionStorage` trait + native ~/.alkanes/
//!                   wc-session.json (mode 0600).
//!   * `signer`    — `WalletConnectSigner<T, S>` driver: pair / restore
//!                   / get_accounts / sign_psbt / sign_message, with
//!                   wake-and-retry on `peer_not_found`.
//!
//! The whole module is gated on `wc-signer` (codec-only, wasm-ok) plus
//! `wc-signer-native` (native transport + file storage). Wasm consumers
//! (alkanes-web-sys, @alkanes/ts-sdk) re-export the wire+crypto+pairing
//! pieces and supply their own transport + storage via the traits.

pub mod crypto;
pub mod pair_wake;
pub mod pairing;
pub mod signer;
pub mod storage;
pub mod transport;
pub mod wire;

pub use signer::{PairInit, WalletConnectSigner, WcError};
pub use storage::{PersistedSession, SessionStorage, StorageError};
pub use transport::{TransportError, WalletPairStream, WalletTransport};
pub use wire::{Plaintext, WireEnvelope};

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use storage::NativeFileStorage;
#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use transport::NativeTransport;
