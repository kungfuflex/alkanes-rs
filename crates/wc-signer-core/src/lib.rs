//! Wasm-clean carve-out of the SUBFROST WalletConnect signer driver.
//!
//! Lifted out of `alkanes-cli-common::wc_signer` in tick-#618 so the
//! browser extension (subfrost-wallet-web-sys) can reuse the same Rust
//! state machine that `alkanes-cli` drives natively, without inheriting
//! `alkanes-cli-common`'s std/native-deps feature unification (which
//! pulls in tokio + reqwest + bitcoincore-rpc + dirs and breaks the
//! wasm32-unknown-unknown link).
//!
//! Module split (mirrors the pre-carve layout 1:1):
//!   * `wire`      — `Plaintext` tagged-union + `WireEnvelope`
//!                   (camelCase to match the TS sender byte-for-byte).
//!   * `crypto`    — X25519 + HKDF-SHA256 + ChaCha20-Poly1305.
//!   * `pairing`   — `subfrost://wc/...` URI build + parse + code/peer
//!                   mint.
//!   * `pair_wake` — `/v1/pair-wake` HTTP wire shapes.
//!   * `transport` — `WalletTransport` trait + `WalletPairStream` trait
//!                   + `TransportError`.
//!   * `storage`   — `SessionStorage` trait + `PersistedSession`
//!                   + `StorageError`.
//!   * `signer`    — `WalletConnectSigner<T, S>` driver: pair / restore
//!                   / get_accounts / sign_psbt / sign_message, with
//!                   wake-and-retry on `peer_not_found`.
//!
//! Native consumers (alkanes-cli, the mobile FFI) carry their own
//! `NativeTransport` (tlsfetch-ws + frtun-pair handshake + reqwest
//! pair-wake POST) and `NativeFileStorage` on top of these traits —
//! see `alkanes-cli-common::wc_signer` for the native re-export.
//!
//! Wasm consumers (subfrost-wallet-web-sys) provide a wasm
//! `WalletTransport` impl backed by `tlsfetch_ws::WsClient` (web_sys
//! backend) and a wasm `SessionStorage` impl backed by IndexedDB.

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
