//! Native WSS dialer — wraps a `tokio-tungstenite` client as a
//! [`BinaryDuplex`] and exposes top-level `dial` / `listen` helpers.
//!
//! Used by the dapp CLI (subfrost-mobile-cli's `pair-listen`
//! subcommand) and the mobile FFI to dial out to a `/v1/pair`
//! bridge over plain WSS. The server-side (subfrost-mobile-api) uses
//! axum's WS layer with its own [`BinaryDuplex`] impl, so it does
//! NOT enable this feature.

use crate::handshake::{handshake_dial, handshake_listen, HandshakeError};
use crate::stream::{BinaryDuplex, PairStream};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::io;
use thiserror::Error;
use tokio_tungstenite::tungstenite::Message;

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

/// Dial a remote peer's listener over `bridge_url` (e.g. `wss://
/// wss-tls.subfrost.io/v1/pair`). Returns a stream the bridge is
/// gluing byte-for-byte between us and the remote peer.
pub async fn dial(
    bridge_url:  &str,
    self_peer:   &str,
    remote_peer: &str,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_dial(TungsteniteDuplex::new(ws), self_peer, remote_peer).await?)
}

/// Listen on `bridge_url` under our own peer name. Returns the
/// stream + the dialer's peer name once an inbound dial arrives.
pub async fn listen(
    bridge_url: &str,
    self_peer:  &str,
) -> Result<PairStream, NativeError> {
    let ws = connect(bridge_url).await?;
    Ok(handshake_listen(TungsteniteDuplex::new(ws), self_peer).await?)
}

async fn connect(bridge_url: &str) -> Result<WsStream, NativeError> {
    let (ws, _resp) = tokio_tungstenite::connect_async(bridge_url).await
        .map_err(|e| NativeError::WsConnect(e.to_string()))?;
    Ok(ws)
}

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
                Some(Ok(Message::Ping(p))) => { let _ = self.inner.send(Message::Pong(p)).await; }
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Text(_))) => return Err(io::Error::new(
                    io::ErrorKind::InvalidData, "unexpected text frame post-handshake")),
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(Message::Frame(_))) => continue,
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
                Some(Ok(Message::Ping(p))) => { let _ = self.inner.send(Message::Pong(p)).await; }
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Binary(_))) => return Err(io::Error::new(
                    io::ErrorKind::InvalidData, "unexpected binary frame during handshake")),
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
