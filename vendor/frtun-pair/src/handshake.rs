//! Run the dial/listen handshake over an already-connected
//! [`BinaryDuplex`].
//!
//! This module is intentionally transport-agnostic — it takes any
//! object that implements `BinaryDuplex` and drives the JSON control
//! protocol from `protocol.rs`. The transport-specific wiring
//! (tokio-tungstenite native, web-sys wasm, in-process mock for tests)
//! lives in sibling modules.

use crate::protocol::{ClientFrame, ServerFrame};
use crate::stream::{BinaryDuplex, PairStream};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HandshakeError {
    #[error("transport: {0}")]
    Transport(#[from] io::Error),
    #[error("bridge closed before handshake completed")]
    BridgeClosed,
    #[error("bridge sent malformed json: {0}")]
    BadJson(String),
    #[error("bridge sent unexpected frame: {0:?}")]
    UnexpectedFrame(ServerFrame),
    #[error("bridge error: [{code}] {msg}")]
    BridgeRejected { code: String, msg: String },
}

/// Run the **dial** handshake. Sends `Dial { peer, self_peer }`,
/// expects `Dialed { peer }` back, and returns the raw byte stream.
pub async fn handshake_dial<D>(
    mut inner: D,
    self_peer:   &str,
    remote_peer: &str,
) -> Result<PairStream, HandshakeError>
where
    D: BinaryDuplex,
{
    let frame = ClientFrame::Dial {
        peer:      remote_peer.to_string(),
        self_peer: self_peer.to_string(),
    };
    inner.send_text(frame.to_json()).await?;

    let resp = inner.recv_text().await?.ok_or(HandshakeError::BridgeClosed)?;
    let resp = ServerFrame::from_json(&resp)
        .map_err(|e| HandshakeError::BadJson(e.to_string()))?;
    match resp {
        ServerFrame::Dialed { peer } => {
            Ok(PairStream::spawn(inner, peer))
        }
        ServerFrame::Error { code, msg } => Err(HandshakeError::BridgeRejected { code, msg }),
        other => Err(HandshakeError::UnexpectedFrame(other)),
    }
}

/// Run the **listen** handshake. Sends `Listen { peer }`, expects
/// `Ready` then later `Incoming { peer }`, and returns the raw byte
/// stream + the dialing peer's name.
pub async fn handshake_listen<D>(
    mut inner: D,
    self_peer: &str,
) -> Result<PairStream, HandshakeError>
where
    D: BinaryDuplex,
{
    inner.send_text(ClientFrame::Listen { peer: self_peer.to_string() }.to_json()).await?;

    let resp = inner.recv_text().await?.ok_or(HandshakeError::BridgeClosed)?;
    match ServerFrame::from_json(&resp)
        .map_err(|e| HandshakeError::BadJson(e.to_string()))?
    {
        ServerFrame::Ready => {}
        ServerFrame::Error { code, msg } => {
            return Err(HandshakeError::BridgeRejected { code, msg });
        }
        other => return Err(HandshakeError::UnexpectedFrame(other)),
    }

    // Now wait for an Incoming.
    let resp = inner.recv_text().await?.ok_or(HandshakeError::BridgeClosed)?;
    match ServerFrame::from_json(&resp)
        .map_err(|e| HandshakeError::BadJson(e.to_string()))?
    {
        ServerFrame::Incoming { peer } => Ok(PairStream::spawn(inner, peer)),
        ServerFrame::Error { code, msg } => Err(HandshakeError::BridgeRejected { code, msg }),
        other => Err(HandshakeError::UnexpectedFrame(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::mock::{pair, MockDuplex, MockFrame};
    use bytes::Bytes;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Spawn a tiny in-process "bridge" that:
    ///   1. accepts a Listen from one side, sends Ready
    ///   2. accepts a Dial from the other side, sends Dialed
    ///   3. sends Incoming to the listener
    ///   4. forwards every binary frame between the two
    ///
    /// Returns the two PairStreams (alice = dialer, bob = listener).
    async fn run_bridge_through(
        dialer_self:    &'static str,
        listener_self:  &'static str,
    ) -> (PairStream, PairStream) {
        let (alice_side, bridge_a) = pair();  // alice ↔ bridge
        let (bob_side,   bridge_b) = pair();  // bob   ↔ bridge

        // Bridge task: drain control frames + bridge binary.
        let bridge_handle = tokio::spawn(async move {
            run_bridge(bridge_a, bridge_b).await;
        });

        // Listener side opens first so the bridge has a "bob" registered.
        let listener_fut = tokio::spawn(async move {
            handshake_listen(bob_side, listener_self).await.unwrap()
        });
        // Tiny pause so the Listen frame is parked before Dial arrives.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        let dialer = handshake_dial(alice_side, dialer_self, listener_self).await.unwrap();
        let listener = listener_fut.await.unwrap();
        // bridge_handle is leaked intentionally — it'll drain when both
        // pair-stream actors close.
        drop(bridge_handle);
        (dialer, listener)
    }

    async fn run_bridge(mut a: MockDuplex, mut b: MockDuplex) {
        // Wait for both sides to send their control frame.
        let mut a_self: Option<String> = None;
        let mut b_self: Option<String> = None;
        let mut a_target_b = false;
        loop {
            tokio::select! {
                f = a.recv_text() => match f {
                    Ok(Some(json)) => {
                        let cf: ClientFrame = ClientFrame::from_json(&json).unwrap();
                        match cf {
                            ClientFrame::Listen { peer } => { a_self = Some(peer); }
                            ClientFrame::Dial   { peer, self_peer } => {
                                a_self = Some(self_peer);
                                a_target_b = peer == b_self.clone().unwrap_or_default();
                                let _ = a.send_text(ServerFrame::Dialed { peer }.to_json()).await;
                            }
                        }
                    }
                    _ => break,
                },
                f = b.recv_text() => match f {
                    Ok(Some(json)) => {
                        let cf: ClientFrame = ClientFrame::from_json(&json).unwrap();
                        match cf {
                            ClientFrame::Listen { peer } => {
                                b_self = Some(peer);
                                let _ = b.send_text(ServerFrame::Ready.to_json()).await;
                            }
                            ClientFrame::Dial { peer, self_peer } => {
                                b_self = Some(self_peer);
                                let _ = b.send_text(ServerFrame::Dialed { peer }.to_json()).await;
                            }
                        }
                    }
                    _ => break,
                },
            }
            if a_self.is_some() && b_self.is_some() && a_target_b {
                // a dialed b — push Incoming to b.
                let _ = b.send_text(
                    ServerFrame::Incoming { peer: a_self.clone().unwrap() }.to_json()
                ).await;
                break;
            }
        }

        // Now bridge raw binary frames in both directions until either
        // side closes.
        loop {
            tokio::select! {
                f = a.recv_binary() => match f {
                    Ok(Some(bytes)) => { let _ = b.send_binary(bytes).await; }
                    _ => break,
                },
                f = b.recv_binary() => match f {
                    Ok(Some(bytes)) => { let _ = a.send_binary(bytes).await; }
                    _ => break,
                },
            }
        }
        let _ = a.close().await;
        let _ = b.close().await;
    }

    #[tokio::test]
    async fn dialer_and_listener_exchange_bytes_through_bridge() {
        let (mut alice, mut bob) =
            run_bridge_through("frtun1alice.peer", "frtun1bob.peer").await;

        // Alice → Bob.
        alice.write_all(b"sign this please").await.unwrap();
        let mut buf = [0u8; 16];
        bob.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"sign this please");

        // Bob → Alice.
        bob.write_all(b"signed").await.unwrap();
        let mut buf = [0u8; 6];
        alice.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"signed");

        // Remote-peer names propagated through the bridge response.
        assert_eq!(alice.remote_peer(), "frtun1bob.peer");
        assert_eq!(bob.remote_peer(),   "frtun1alice.peer");
    }

    #[tokio::test]
    async fn dial_bridge_error_surfaces() {
        // Use a hand-crafted duplex that returns Error to the dial.
        let (mut sock, mut bridge) = pair();
        tokio::spawn(async move {
            let _ = bridge.recv_text().await; // consume the Dial
            let _ = bridge.send_text(
                ServerFrame::Error {
                    code: crate::protocol::codes::PEER_NOT_FOUND.into(),
                    msg:  "peer offline".into(),
                }.to_json()
            ).await;
        });
        let err = handshake_dial(sock, "frtun1alice.peer", "frtun1bob.peer").await.unwrap_err();
        match err {
            HandshakeError::BridgeRejected { code, msg } => {
                assert_eq!(code, crate::protocol::codes::PEER_NOT_FOUND);
                assert_eq!(msg, "peer offline");
            }
            _ => panic!("expected BridgeRejected, got {err:?}"),
        }
    }

    #[tokio::test]
    async fn listen_bridge_close_before_incoming() {
        let (sock, mut bridge) = pair();
        tokio::spawn(async move {
            let _ = bridge.recv_text().await; // consume the Listen
            let _ = bridge.send_text(ServerFrame::Ready.to_json()).await;
            let _ = bridge.close().await;
        });
        let err = handshake_listen(sock, "frtun1bob.peer").await.unwrap_err();
        assert!(matches!(err, HandshakeError::BridgeClosed));
    }

    // Suppress unused-import warning on Bytes/MockFrame when only some
    // tests use them.
    #[allow(dead_code)]
    fn _refs(_: Bytes, _: MockFrame) {}
}
