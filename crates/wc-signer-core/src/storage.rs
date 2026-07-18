//! `SessionStorage` trait + `PersistedSession` wire shape.
//!
//! What gets persisted per pair:
//!   * our X25519 priv + pub (so we can re-derive symKey after restart)
//!   * the phone's X25519 pub
//!   * the symKey itself (cached so we don't ECDH every restart)
//!   * the bridge URL, the CLI peer name, the phone peer name, the
//!     pairing code, the wallet's address list, origin
//!
//! Native consumers wrap this trait around plain JSON on
//! `~/.alkanes/wc-session.json` (mode 0600) — see
//! `alkanes-cli-common::wc_signer::storage::NativeFileStorage`.
//! Wasm consumers wrap it around IndexedDB — see
//! `subfrost-wallet-web-sys::wc_signer::WasmStorage`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The persisted session — what `restore()` rehydrates from. The
/// keystore-style envelope is intentionally minimal so `subfrost-app`
/// and `alkanes-cli` agree on the disk shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    /// Our CLI's identity (bech32-ish peer name + b64url-32B secret).
    pub cli_peer_name: String,
    /// The phone's peer name that dialed us during pair.
    pub wallet_peer_name: String,
    /// Bridge URL we paired against (e.g. wss://wss-tls.subfrost.io/v1/pair).
    pub bridge_url: String,
    /// Origin string the wallet showed the user at pair time.
    pub origin: String,
    /// 6-char pairing code the user typed.
    pub pairing_code: String,
    /// b64url-encoded 32-byte symKey (HKDF output of the ECDH).
    pub sym_key_b64: String,
    /// b64url X25519 own priv (32B), for re-deriving if needed.
    pub own_priv_b64: String,
    /// b64url X25519 own pub (32B).
    pub own_pub_b64: String,
    /// b64url X25519 wallet pub (32B).
    pub peer_pub_b64: String,
    /// Cached wallet address list — populated by `get_accounts()` after
    /// pair so we don't have to round-trip on every sign.
    #[serde(default)]
    pub accounts: Vec<String>,
    /// ISO 8601 paired-at timestamp.
    pub paired_at: String,
    /// ISO 8601 last-used-at timestamp.
    pub last_used_at: String,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("io: {0}")]
    Io(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("not found")]
    NotFound,
}

// Same Send-bound story as `WalletTransport` / `BinaryDuplex`: native
// keeps `Send + Sync` (the signer state machine is driven from
// tokio-spawned tasks); wasm32 drops the bounds because the browser
// runtime is single-threaded and IndexedDB request callbacks are
// `!Send`.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn save(&self, session: &PersistedSession) -> Result<(), StorageError>;
    async fn load(&self) -> Result<Option<PersistedSession>, StorageError>;
    async fn delete(&self) -> Result<(), StorageError>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait SessionStorage {
    async fn save(&self, session: &PersistedSession) -> Result<(), StorageError>;
    async fn load(&self) -> Result<Option<PersistedSession>, StorageError>;
    async fn delete(&self) -> Result<(), StorageError>;
}
