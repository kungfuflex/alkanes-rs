//! `WalletTransport` trait + native impl.
//!
//! The transport is the dial / listen / read / write surface the signer
//! needs to talk to the SUBFROST mobile phone over the bridge. Native
//! impl wraps `frtun_pair::client_native` (tokio-tungstenite WSS dial)
//! and pokes `/v1/pair-wake` via the HTTP twin endpoint.
//!
//! Wasm impl lives in `alkanes-web-sys::wc_signer` — different I/O
//! primitive (browser `WebSocket` + `fetch`) but same trait shape.

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("dial: {0}")]
    Dial(String),
    #[error("listen: {0}")]
    Listen(String),
    #[error("io: {0}")]
    Io(String),
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("peer_not_found")]
    PeerNotFound,
    #[error("http: {0}")]
    Http(String),
}

impl TransportError {
    pub fn is_peer_not_found(&self) -> bool {
        match self {
            TransportError::PeerNotFound => true,
            TransportError::Dial(s) | TransportError::Io(s) | TransportError::Listen(s) => {
                s.contains("peer_not_found")
            }
            _ => false,
        }
    }
}

/// One open dial-or-listen connection to the bridge.
#[async_trait]
pub trait WalletPairStream: Send {
    async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError>;
    async fn close(&mut self);
    /// The remote peer name we're talking to (post-handshake).
    fn remote_peer(&self) -> &str;
}

/// Transport-level routing surface.
#[async_trait]
pub trait WalletTransport: Send + Sync {
    /// `dial` a remote peer's listening session on the bridge.
    async fn dial(
        &self,
        bridge_url: &str,
        self_peer: &str,
        remote_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError>;

    /// `listen` for an inbound dial under our own peer name.
    async fn listen(
        &self,
        bridge_url: &str,
        self_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError>;

    /// POST `/v1/pair-wake` on the bridge's HTTP twin. Returns
    /// `Ok(delivered)` so callers can decide whether to retry.
    async fn pair_wake(&self, bridge_url: &str, peer: &str) -> Result<bool, TransportError>;
}

// =====================================================================
// Native impl
// =====================================================================

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use native::NativeTransport;

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
mod native {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub struct NativeTransport;

    impl NativeTransport {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for NativeTransport {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Adapter wrapping `frtun_pair::PairStream` as a `WalletPairStream`.
    struct PairStreamAdapter {
        inner: frtun_pair::PairStream,
        remote: String,
    }

    #[async_trait]
    impl WalletPairStream for PairStreamAdapter {
        async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            self.inner
                .write_all(bytes)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))
        }

        async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError> {
            let mut buf = vec![0u8; 65536];
            let n = self
                .inner
                .read(&mut buf)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
            if n == 0 {
                return Err(TransportError::Io("eof".into()));
            }
            buf.truncate(n);
            Ok(buf)
        }

        async fn close(&mut self) {
            let _ = self.inner.shutdown().await;
        }

        fn remote_peer(&self) -> &str {
            &self.remote
        }
    }

    #[async_trait]
    impl WalletTransport for NativeTransport {
        async fn dial(
            &self,
            bridge_url: &str,
            self_peer: &str,
            remote_peer: &str,
        ) -> Result<Box<dyn WalletPairStream>, TransportError> {
            let stream = frtun_pair::client_native::dial(bridge_url, self_peer, remote_peer)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("peer_not_found") {
                        TransportError::PeerNotFound
                    } else {
                        TransportError::Dial(msg)
                    }
                })?;
            let remote = stream.remote_peer().to_string();
            Ok(Box::new(PairStreamAdapter { inner: stream, remote }))
        }

        async fn listen(
            &self,
            bridge_url: &str,
            self_peer: &str,
        ) -> Result<Box<dyn WalletPairStream>, TransportError> {
            let stream = frtun_pair::client_native::listen(bridge_url, self_peer)
                .await
                .map_err(|e| TransportError::Listen(e.to_string()))?;
            let remote = stream.remote_peer().to_string();
            Ok(Box::new(PairStreamAdapter { inner: stream, remote }))
        }

        async fn pair_wake(
            &self,
            bridge_url: &str,
            peer: &str,
        ) -> Result<bool, TransportError> {
            let http_base = wss_to_https_base(bridge_url);
            let url = format!("{}/v1/pair-wake", http_base);
            let body = serde_json::json!({ "peer": peer });
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| TransportError::Http(e.to_string()))?;
            let status = resp.status();
            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| TransportError::Http(format!("decode wake response: {e}")))?;
            if !status.is_success() {
                return Err(TransportError::Http(format!(
                    "{status} {}",
                    json.get("reason").and_then(|v| v.as_str()).unwrap_or("")
                )));
            }
            Ok(json
                .get("delivered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
        }
    }

    /// Strip `/v1/pair` suffix + flip scheme. Mirrors the TS helper.
    fn wss_to_https_base(wss: &str) -> String {
        let base = if let Some(rest) = wss.strip_prefix("wss://") {
            format!("https://{}", rest)
        } else if let Some(rest) = wss.strip_prefix("ws://") {
            format!("http://{}", rest)
        } else {
            wss.to_string()
        };
        // Drop /v1/pair if present.
        if let Some(idx) = base.find("/v1/pair") {
            base[..idx].to_string()
        } else {
            base.trim_end_matches('/').to_string()
        }
    }

    #[cfg(test)]
    mod native_tests {
        use super::*;

        #[test]
        fn wss_url_to_http_base() {
            assert_eq!(
                wss_to_https_base("wss://wss-tls.subfrost.io/v1/pair"),
                "https://wss-tls.subfrost.io"
            );
            assert_eq!(
                wss_to_https_base("ws://127.0.0.1:18801/v1/pair"),
                "http://127.0.0.1:18801"
            );
        }
    }
}
