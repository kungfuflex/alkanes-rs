//! `BinaryDuplex` trait — wasm-clean carve-out of frtun-pair's
//! transport-abstraction surface. Same trait shape as the canonical
//! [`frtun_pair::stream::BinaryDuplex`] — the codec-only version drops
//! the tokio-actor `PairStream::spawn(...)` post-handshake driver,
//! since wasm consumers can't link tokio.
//!
//! Native consumers (alkanes-cli-common's NativeTransport, the axum
//! bridge) wrap the actor on top via `frtun-pair::PairStream::spawn`.
//! Wasm consumers (subfrost-wallet-web-sys) pump bytes directly out
//! of the duplex via `wasm_bindgen_futures::spawn_local`.

use async_trait::async_trait;
use bytes::Bytes;
use std::io;

// The codec trait carries a `Send + 'static` bound on native to keep
// it compatible with tokio's `spawn`. On wasm32 the runtime is
// single-threaded and `web_sys::WebSocket`-backed types are `!Send`;
// we drop the bound there. The `async_trait` attribute mirrors the
// same gate.

/// A bidirectional channel of binary WebSocket-like frames.
///
/// Native uses tokio-tungstenite's `WebSocketStream`, tests use a
/// `tokio::sync::mpsc`-backed mock, the wasm port wraps the
/// browser `WebSocket` via `web_sys`. All three plug in here.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait BinaryDuplex: Send + 'static {
    /// Send one binary frame.
    async fn send_binary(&mut self, data: Bytes) -> io::Result<()>;
    /// Receive one binary frame. `Ok(None)` means the peer closed.
    async fn recv_binary(&mut self) -> io::Result<Option<Bytes>>;
    /// Send a text frame (handshake JSON).
    async fn send_text(&mut self, text: String) -> io::Result<()>;
    /// Receive one frame as text. Returns None on close.
    async fn recv_text(&mut self) -> io::Result<Option<String>>;
    /// Best-effort close.
    async fn close(&mut self) -> io::Result<()>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait BinaryDuplex: 'static {
    async fn send_binary(&mut self, data: Bytes) -> io::Result<()>;
    async fn recv_binary(&mut self) -> io::Result<Option<Bytes>>;
    async fn send_text(&mut self, text: String) -> io::Result<()>;
    async fn recv_text(&mut self) -> io::Result<Option<String>>;
    async fn close(&mut self) -> io::Result<()>;
}

/// Boxed `BinaryDuplex` for return-by-trait sites.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxedBinaryDuplex = Box<dyn BinaryDuplex>;
#[cfg(target_arch = "wasm32")]
pub type BoxedBinaryDuplex = Box<dyn BinaryDuplex>;
