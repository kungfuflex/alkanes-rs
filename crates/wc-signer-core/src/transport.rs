//! `WalletTransport` + `WalletPairStream` traits.
//!
//! The transport is the dial / listen / read / write surface the signer
//! needs to talk to the SUBFROST mobile phone over the bridge. Native
//! impl lives in `alkanes-cli-common::wc_signer::transport::NativeTransport`
//! (tlsfetch-ws + frtun-pair handshake + reqwest pair-wake POST). Wasm
//! impl lives in `subfrost-wallet-web-sys::wc_signer::WasmTransport`
//! (web_sys WebSocket + Fetch). Both targets share the same signer
//! state machine in `signer.rs`.

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
    /// Fast-fail reachability probe (`PairStream::ping`) failed — peer
    /// is either gone or unreachable. Only surfaced when the `icmp`
    /// feature is enabled on both sides; with the flag off, ping is a
    /// no-op and this variant is unused.
    #[cfg(feature = "icmp")]
    #[error("ping: {0}")]
    Ping(String),
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

// Same Send-bound story as `frtun-pair-codec::BinaryDuplex`: native
// keeps `Send`/`Send + Sync` (the signer state machine drives this
// inside tokio-spawned tasks); wasm32 drops those bounds because the
// browser runtime is single-threaded and `web_sys::WebSocket`-backed
// types are `!Send`.

/// One open dial-or-listen connection to the bridge.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait WalletPairStream: Send {
    async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError>;
    async fn close(&mut self);
    /// The remote peer name we're talking to (post-handshake).
    fn remote_peer(&self) -> &str;
    /// Fast-fail reachability probe — round-trip a PING frame; return
    /// the measured RTT. Use this immediately after dial to surface
    /// "peer unreachable" in seconds instead of waiting the full
    /// request timeout. **Only available with the `icmp` feature on**;
    /// the wire-shape change requires both peers to agree.
    #[cfg(feature = "icmp")]
    async fn ping(&mut self, timeout: Duration) -> Result<Duration, TransportError>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait WalletPairStream {
    async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError>;
    async fn close(&mut self);
    fn remote_peer(&self) -> &str;
    #[cfg(feature = "icmp")]
    async fn ping(&mut self, timeout: Duration) -> Result<Duration, TransportError>;
}

/// Transport-level routing surface.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait WalletTransport {
    async fn dial(
        &self,
        bridge_url: &str,
        self_peer: &str,
        remote_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError>;

    async fn listen(
        &self,
        bridge_url: &str,
        self_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError>;

    async fn pair_wake(&self, bridge_url: &str, peer: &str) -> Result<bool, TransportError>;
}
