//! `SessionStorage` trait + native (~/.alkanes/wc-session.json, 0600)
//! impl.
//!
//! What gets persisted per pair:
//!   * our X25519 priv + pub (so we can re-derive symKey after restart)
//!   * the phone's X25519 pub
//!   * the symKey itself (cached so we don't ECDH every restart)
//!   * the bridge URL, the CLI peer name, the phone peer name, the
//!     pairing code, the wallet's address list, origin
//!
//! Format choice: plain JSON with restrictive file perms. The session
//! is only useful when paired against the matching phone, so the
//! security model is "don't leak the .json off the host".

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

#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn save(&self, session: &PersistedSession) -> Result<(), StorageError>;
    async fn load(&self) -> Result<Option<PersistedSession>, StorageError>;
    async fn delete(&self) -> Result<(), StorageError>;
}

// =====================================================================
// Native file-backed impl — ~/.alkanes/wc-session.json, mode 0600.
// =====================================================================

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use native::NativeFileStorage;

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
mod native {
    use super::*;
    use std::path::PathBuf;

    pub struct NativeFileStorage {
        path: PathBuf,
    }

    impl NativeFileStorage {
        /// Default ~/.alkanes/wc-session.json. Override env var
        /// `ALKANES_WC_SESSION_PATH` for testing.
        pub fn default_path() -> Self {
            if let Ok(p) = std::env::var("ALKANES_WC_SESSION_PATH") {
                return Self { path: PathBuf::from(p) };
            }
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."));
            let path = home.join(".alkanes").join("wc-session.json");
            Self { path }
        }

        pub fn new(path: PathBuf) -> Self {
            Self { path }
        }

        pub fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    #[async_trait]
    impl SessionStorage for NativeFileStorage {
        async fn save(&self, session: &PersistedSession) -> Result<(), StorageError> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StorageError::Io(format!("mkdir {}: {e}", parent.display())))?;
            }
            let json = serde_json::to_string_pretty(session)
                .map_err(|e| StorageError::Parse(e.to_string()))?;
            std::fs::write(&self.path, json)
                .map_err(|e| StorageError::Io(format!("write {}: {e}", self.path.display())))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&self.path)
                    .map_err(|e| StorageError::Io(e.to_string()))?
                    .permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(&self.path, perms)
                    .map_err(|e| StorageError::Io(e.to_string()))?;
            }
            Ok(())
        }

        async fn load(&self) -> Result<Option<PersistedSession>, StorageError> {
            if !self.path.exists() {
                return Ok(None);
            }
            let raw = std::fs::read_to_string(&self.path)
                .map_err(|e| StorageError::Io(format!("read {}: {e}", self.path.display())))?;
            let s: PersistedSession = serde_json::from_str(&raw)
                .map_err(|e| StorageError::Parse(e.to_string()))?;
            Ok(Some(s))
        }

        async fn delete(&self) -> Result<(), StorageError> {
            if self.path.exists() {
                std::fs::remove_file(&self.path)
                    .map_err(|e| StorageError::Io(format!("rm {}: {e}", self.path.display())))?;
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn sample_session() -> PersistedSession {
            PersistedSession {
                cli_peer_name: "frtun1abc.peer".into(),
                wallet_peer_name: "frtun1def.peer".into(),
                bridge_url: "wss://wss-tls.subfrost.io/v1/pair".into(),
                origin: "cli://test".into(),
                pairing_code: "ABCDEF".into(),
                sym_key_b64: "AAAA".into(),
                own_priv_b64: "BBBB".into(),
                own_pub_b64: "CCCC".into(),
                peer_pub_b64: "DDDD".into(),
                accounts: vec!["bc1qa".into()],
                paired_at: "2026-06-09T00:00:00Z".into(),
                last_used_at: "2026-06-09T00:00:00Z".into(),
            }
        }

        #[tokio::test]
        async fn save_load_delete_round_trip() {
            let tmp = tempfile::NamedTempFile::new().unwrap();
            let path = tmp.path().to_path_buf();
            drop(tmp); // we just want the path
            let storage = NativeFileStorage::new(path.clone());

            let s = sample_session();
            storage.save(&s).await.unwrap();
            let loaded = storage.load().await.unwrap().expect("session present");
            assert_eq!(loaded.cli_peer_name, s.cli_peer_name);
            assert_eq!(loaded.accounts, s.accounts);

            storage.delete().await.unwrap();
            let after = storage.load().await.unwrap();
            assert!(after.is_none(), "delete must remove the file");
        }
    }
}
