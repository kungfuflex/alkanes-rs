//! Cross-platform async WSS client.
//!
//! Tick 1 of a 5-tick architectural upgrade. Native today, wasm in
//! tick 3.
//!
//! ## Architecture
//!
//! The native backend wraps `tokio-tungstenite` with its built-in
//! `rustls-tls-webpki-roots` feature. This crate is intentionally
//! TLS-engine-agnostic — it does NOT depend on `rustls-tlsfetch`,
//! the JA3-emulating rustls fork that lives in this workspace.
//! Downstream consumers (e.g. alkanes-rs-develop in tick 2) can
//! vendor `tlsfetch-ws` without inheriting the
//! `[patch.crates-io] rustls = ...` surgery the fingerprint stack
//! requires. The WC pairing rendezvous use case this crate targets
//! has no fingerprint requirement so unforked rustls is correct.
//!
//! ## Wire shape
//!
//! ```ignore
//! use tlsfetch_ws::{WsClient, WsConnectOptions, WsMessage};
//!
//! # async fn demo() -> Result<(), tlsfetch_transport::TransportError> {
//! let mut ws = WsClient::connect(
//!     "wss://wss-tls.subfrost.io/v1/pair",
//!     &WsConnectOptions::default(),
//! ).await?;
//! ws.send_binary(bytes::Bytes::from_static(b"hello")).await?;
//! match ws.recv().await? {
//!     Some(WsMessage::Binary(b)) => println!("got {} bytes", b.len()),
//!     Some(WsMessage::Text(t))   => println!("got text: {t}"),
//!     Some(WsMessage::Ping(_) | WsMessage::Pong(_)) => {}
//!     None => println!("clean close"),
//! }
//! ws.close(1000, "bye").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Forward-compat fields
//!
//! [`WsConnectOptions::spki_pins`] and [`WsConnectOptions::insecure`]
//! are part of the public API surface for tick-1 callers to populate,
//! but they are *not yet wired into verification*. Setting either logs
//! a TODO and falls through to webpki-roots verification. Real pinning
//! lands when (and if) any consumer of `tlsfetch-ws` needs it; the
//! current target (WC pairing rendezvous) does not.

use bytes::Bytes;
use std::time::Duration;

#[cfg(feature = "native")]
mod native;

#[cfg(feature = "native")]
pub use native::WsClient;

/// Re-export of the underlying transport error so consumers can
/// import it without having to add a direct `tlsfetch-transport`
/// dependency. The native `WsClient::recv` / `send_*` methods return
/// `Result<_, TransportError>`; we surface that type here for
/// downstream pattern-matching (e.g. wrapping ourselves around the
/// duplex with our own keepalive loop).
pub use tlsfetch_transport::TransportError;

/// Options for dialing a WebSocket endpoint.
///
/// All fields are optional. The empty `Default` shape is sufficient
/// for a plaintext `ws://...` dial against a server with no
/// subprotocol negotiation and no authentication headers.
#[derive(Default, Clone, Debug)]
pub struct WsConnectOptions {
    /// `Sec-WebSocket-Protocol` candidates the server may choose from.
    /// Sent as a comma-separated header value.
    pub subprotocols: Vec<String>,
    /// Extra HTTP/1.1 request headers carried in the WS handshake.
    /// `Host`, `Upgrade`, `Connection`, `Sec-WebSocket-Key`,
    /// `Sec-WebSocket-Version` are reserved and will be stripped.
    pub headers: Vec<(String, String)>,
    /// If true, skip cert verification. **Tick 1: logged + ignored.**
    pub insecure: bool,
    /// SPKI pin set (raw SHA-256 of SubjectPublicKeyInfo DER).
    /// **Tick 1: logged + ignored** — webpki-roots verification only.
    pub spki_pins: Option<Vec<[u8; 32]>>,
    /// Overall connect timeout (TCP + TLS + WS handshake).
    pub connect_timeout: Option<Duration>,
}

/// One WebSocket frame surface that the client emits or accepts.
#[derive(Debug, Clone)]
pub enum WsMessage {
    /// Binary payload — common for any binary protocol on top of WS.
    Binary(Bytes),
    /// UTF-8 text payload.
    Text(String),
    /// Peer-originated ping. Auto-ponged by the underlying codec; the
    /// payload is surfaced for diagnostics only.
    Ping(Bytes),
    /// Peer-originated pong reply.
    Pong(Bytes),
}
