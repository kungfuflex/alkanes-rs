//! Read-through upstream KV source.
//!
//! When the WASM calls `__get`/`__get_len` and the in-memory storage
//! doesn't have a key, the host functions consult a [`ForkUpstream`]
//! to fetch the value from a confirmed-state indexer (typically
//! `metashrew_view "getstorageat"` JSON-RPC against
//! `https://mainnet.subfrost.io/v4/subfrost`).
//!
//! The trait is async because the canonical upstream is over HTTP;
//! for tests and tight loops, [`testing::StubUpstream`] is a sync
//! HashMap-backed impl.

use async_trait::async_trait;

/// An async key-value source. Implementors typically wrap a JSON-RPC
/// client that exposes the indexer's confirmed key-value store.
///
/// `fetch` distinguishes:
///   * `Ok(Some(v))` — upstream knows the key and returned `v`.
///   * `Ok(None)`    — upstream confirms the key is genuinely absent.
///   * `Err(e)`      — transient or permanent failure (network, parse,
///                     auth). Callers may surface this to WASM as an
///                     "absent" key (matches the empty-projection
///                     semantics for novel outpoints) or propagate.
#[async_trait]
pub trait ForkUpstream: Send + Sync {
    /// Fetch the value associated with `key`, if any.
    async fn fetch(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String>;

    /// Upstream's current confirmed tip height. Used to derive the
    /// projected tip (`tip + 1`) when constructing fork-mode storage.
    async fn tip_height(&self) -> Result<u32, String> {
        Ok(0)
    }
}

// -- HTTP impl ---------------------------------------------------------------
//
// Calls metashrew_view "getstorageat" via JSON-RPC. The payload format
// is the standard JSON-RPC 2.0 envelope; the result is the hex-encoded
// value bytes (or null for missing).

#[cfg(feature = "http-upstream")]
pub use http::HttpForkUpstream;

#[cfg(feature = "http-upstream")]
mod http {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// HTTP-backed upstream that calls `metashrew_view "getstorageat"`.
    ///
    /// Default URL: `https://mainnet.subfrost.io/v4/subfrost`. Override
    /// via [`HttpForkUpstream::with_url`].
    pub struct HttpForkUpstream {
        client: reqwest::Client,
        url: String,
    }

    impl HttpForkUpstream {
        pub fn new() -> Result<Self, String> {
            Self::with_url("https://mainnet.subfrost.io/v4/subfrost".into())
        }

        pub fn with_url(url: String) -> Result<Self, String> {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| format!("HttpForkUpstream client: {}", e))?;
            Ok(Self { client, url })
        }

        pub fn url(&self) -> &str {
            &self.url
        }
    }

    #[derive(Serialize)]
    struct JsonRpcReq<'a> {
        jsonrpc: &'a str,
        id: u32,
        method: &'a str,
        params: Vec<serde_json::Value>,
    }

    #[derive(Deserialize)]
    struct JsonRpcResp {
        #[serde(default)]
        result: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<serde_json::Value>,
    }

    #[async_trait]
    impl ForkUpstream for HttpForkUpstream {
        async fn fetch(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
            let key_hex = format!("0x{}", hex::encode(key));
            let req = JsonRpcReq {
                jsonrpc: "2.0",
                id: 0,
                method: "metashrew_view",
                params: vec![
                    serde_json::Value::String("getstorageat".into()),
                    serde_json::Value::String(key_hex),
                    serde_json::Value::String("latest".into()),
                ],
            };
            let resp: JsonRpcResp = self
                .client
                .post(&self.url)
                .json(&req)
                .send()
                .await
                .map_err(|e| format!("upstream send: {}", e))?
                .json()
                .await
                .map_err(|e| format!("upstream parse: {}", e))?;

            if let Some(err) = resp.error {
                return Err(format!("upstream rpc error: {}", err));
            }
            match resp.result {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(serde_json::Value::String(s)) => {
                    let trimmed = s.trim_start_matches("0x");
                    if trimmed.is_empty() {
                        return Ok(Some(Vec::new()));
                    }
                    let bytes = hex::decode(trimmed)
                        .map_err(|e| format!("upstream decode hex: {}", e))?;
                    Ok(Some(bytes))
                }
                Some(other) => Err(format!("upstream unexpected result: {}", other)),
            }
        }

        async fn tip_height(&self) -> Result<u32, String> {
            let req = JsonRpcReq {
                jsonrpc: "2.0",
                id: 0,
                method: "metashrew_height",
                params: vec![],
            };
            let resp: JsonRpcResp = self
                .client
                .post(&self.url)
                .json(&req)
                .send()
                .await
                .map_err(|e| format!("tip send: {}", e))?
                .json()
                .await
                .map_err(|e| format!("tip parse: {}", e))?;

            if let Some(err) = resp.error {
                return Err(format!("tip rpc error: {}", err));
            }
            match resp.result {
                Some(serde_json::Value::Number(n)) => n
                    .as_u64()
                    .map(|x| x as u32)
                    .ok_or_else(|| "tip not u64".into()),
                Some(serde_json::Value::String(s)) => s
                    .trim_start_matches("0x")
                    .parse::<u32>()
                    .or_else(|_| {
                        u32::from_str_radix(s.trim_start_matches("0x"), 16)
                            .map_err(|e| format!("tip parse {}: {}", s, e))
                    })
                    .map_err(|e| format!("tip parse: {}", e)),
                _ => Err("tip missing".into()),
            }
        }
    }
}

// -- Test stubs --------------------------------------------------------------

pub mod testing {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// HashMap-backed upstream for tests. `Ok(Some(v))` for known keys,
    /// `Ok(None)` for absent keys. Records every fetch call for assertions.
    pub struct StubUpstream {
        kv: HashMap<Vec<u8>, Vec<u8>>,
        tip: u32,
        fetch_count: Arc<AtomicUsize>,
    }

    impl StubUpstream {
        pub fn new(kv: HashMap<Vec<u8>, Vec<u8>>) -> Self {
            Self {
                kv,
                tip: 0,
                fetch_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        pub fn with_tip(mut self, tip: u32) -> Self {
            self.tip = tip;
            self
        }

        pub fn fetch_counter(&self) -> Arc<AtomicUsize> {
            self.fetch_count.clone()
        }
    }

    #[async_trait]
    impl ForkUpstream for StubUpstream {
        async fn fetch(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
            self.fetch_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.kv.get(key).cloned())
        }

        async fn tip_height(&self) -> Result<u32, String> {
            Ok(self.tip)
        }
    }

    /// Upstream that always errors. Used to verify error paths.
    pub struct ErrorUpstream {
        pub message: String,
    }

    #[async_trait]
    impl ForkUpstream for ErrorUpstream {
        async fn fetch(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, String> {
            Err(self.message.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{ErrorUpstream, StubUpstream};
    use super::ForkUpstream;
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn stub_returns_configured_value() {
        let mut kv = HashMap::new();
        kv.insert(b"hello".to_vec(), b"world".to_vec());
        let up = StubUpstream::new(kv);
        let r = rt();
        let v = r.block_on(up.fetch(b"hello")).unwrap();
        assert_eq!(v, Some(b"world".to_vec()));
    }

    #[test]
    fn stub_returns_none_for_missing_key() {
        let up = StubUpstream::new(HashMap::new());
        let r = rt();
        assert!(r.block_on(up.fetch(b"absent")).unwrap().is_none());
    }

    #[test]
    fn stub_records_fetch_count() {
        let up = StubUpstream::new(HashMap::new());
        let counter = up.fetch_counter();
        let r = rt();
        r.block_on(up.fetch(b"a")).unwrap();
        r.block_on(up.fetch(b"b")).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stub_tip_is_configurable() {
        let up = StubUpstream::new(HashMap::new()).with_tip(99);
        let r = rt();
        assert_eq!(r.block_on(up.tip_height()).unwrap(), 99);
    }

    #[test]
    fn error_upstream_propagates_error() {
        let up = ErrorUpstream {
            message: "boom".into(),
        };
        let r = rt();
        let err = r.block_on(up.fetch(b"k")).unwrap_err();
        assert_eq!(err, "boom");
    }
}
