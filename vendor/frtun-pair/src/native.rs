//! Native WebSocket connector — wraps `tokio-tungstenite` as a
//! [`BinaryDuplex`] and exposes top-level `dial` / `listen` functions.
//!
//! Wasm targets get their own connector (`frtun-pair-wasm`) that uses
//! `web-sys::WebSocket` instead. The handshake logic and stream layer
//! are shared between both targets.

use crate::handshake::{handshake_dial, handshake_listen, HandshakeError};
use crate::stream::{BinaryDuplex, PairStream};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::io;
use thiserror::Error;
use tokio_tungstenite::tungstenite::Message;

#[cfg(feature = "identity-reexport")]
use frtun_identity::PeerName;

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

#[derive(Debug, Error)]
pub enum NativeError {
    #[error("websocket connect: {0}")]
    WsConnect(String),
    #[error("handshake: {0}")]
    Handshake(#[from] HandshakeError),
}

/// Dial a remote peer over a frtun-pair bridge. Returns a stream that
/// the bridge is gluing byte-for-byte between us and the remote peer.
///
/// Peer-name argument shape:
/// - `feature = "identity-reexport"` (canonical default): typed
///   `&PeerName` from `frtun-identity`. Auto-stringified before going on
///   the wire.
/// - `feature = "identity-reexport"` OFF (vendored copies in
///   subfrost-mobile / alkanes-rs-develop, which don't ship
///   `frtun-identity`): plain `&str` peer-name. Same wire shape — the
///   caller is responsible for producing a bech32m string upstream.
#[cfg(feature = "identity-reexport")]
pub async fn dial(
    bridge_url:  &str,
    self_peer:   &PeerName,
    remote_peer: &PeerName,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_dial(
        TungsteniteDuplex::new(ws),
        &self_peer.to_string(),
        &remote_peer.to_string(),
    ).await?)
}

#[cfg(not(feature = "identity-reexport"))]
pub async fn dial(
    bridge_url:  &str,
    self_peer:   &str,
    remote_peer: &str,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_dial(
        TungsteniteDuplex::new(ws),
        self_peer,
        remote_peer,
    ).await?)
}

/// Listen on a frtun-pair bridge under our own PeerName, returning
/// the first inbound dial's stream.
///
/// See [`dial`] for the `&PeerName` vs `&str` rationale.
#[cfg(feature = "identity-reexport")]
pub async fn listen(
    bridge_url: &str,
    self_peer:  &PeerName,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_listen(
        TungsteniteDuplex::new(ws),
        &self_peer.to_string(),
    ).await?)
}

#[cfg(not(feature = "identity-reexport"))]
pub async fn listen(
    bridge_url: &str,
    self_peer:  &str,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_listen(
        TungsteniteDuplex::new(ws),
        self_peer,
    ).await?)
}

async fn connect(bridge_url: &str) -> Result<WsStream, NativeError> {
    let (ws, _resp) = tokio_tungstenite::connect_async(bridge_url).await
        .map_err(|e| NativeError::WsConnect(e.to_string()))?;
    Ok(ws)
}

// --- BinaryDuplex impl over tungstenite WebSocketStream --------------

struct TungsteniteDuplex {
    inner: WsStream,
}

impl TungsteniteDuplex {
    fn new(inner: WsStream) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl BinaryDuplex for TungsteniteDuplex {
    async fn send_binary(&mut self, data: Bytes) -> io::Result<()> {
        self.inner.send(Message::Binary(data.to_vec().into())).await
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
    }

    async fn recv_binary(&mut self) -> io::Result<Option<Bytes>> {
        loop {
            match self.inner.next().await {
                Some(Ok(Message::Binary(b))) => return Ok(Some(b.to_vec().into())),
                Some(Ok(Message::Ping(p))) => {
                    // Tungstenite handles pong automatically per default
                    // config, but defensively reply.
                    let _ = self.inner.send(Message::Pong(p)).await;
                    continue;
                }
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Text(_))) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected text frame after handshake",
                    ));
                }
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(Message::Frame(_))) => continue, // raw frames not surfaced
                Some(Err(e)) => return Err(io::Error::new(io::ErrorKind::Other, e)),
                None => return Ok(None),
            }
        }
    }

    async fn send_text(&mut self, text: String) -> io::Result<()> {
        self.inner.send(Message::Text(text.into())).await
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
    }

    async fn recv_text(&mut self) -> io::Result<Option<String>> {
        loop {
            match self.inner.next().await {
                Some(Ok(Message::Text(s))) => return Ok(Some(s.to_string())),
                Some(Ok(Message::Ping(p))) => {
                    let _ = self.inner.send(Message::Pong(p)).await;
                    continue;
                }
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Binary(_))) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected binary frame during handshake",
                    ));
                }
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(Message::Frame(_))) => continue,
                Some(Err(e)) => return Err(io::Error::new(io::ErrorKind::Other, e)),
                None => return Ok(None),
            }
        }
    }

    async fn close(&mut self) -> io::Result<()> {
        let _ = self.inner.close(None).await;
        Ok(())
    }
}
