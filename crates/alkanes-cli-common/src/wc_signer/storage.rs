//! Native file-backed `SessionStorage` impl —
//! `~/.alkanes/wc-session.json`, mode 0600.
//!
//! The trait + `PersistedSession` wire shape now live in
//! `wc_signer_core::storage`; this file is the native-only impl on top.
//! Re-export the core types here so existing
//! `alkanes_cli_common::wc_signer::storage::{PersistedSession,
//! SessionStorage, StorageError}` import paths keep compiling.

#[cfg(feature = "wc-signer")]
pub use wc_signer_core::storage::{PersistedSession, SessionStorage, StorageError};

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use native::NativeFileStorage;

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
mod native {
    use async_trait::async_trait;
    use std::path::PathBuf;
    use wc_signer_core::storage::{PersistedSession, SessionStorage, StorageError};

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
