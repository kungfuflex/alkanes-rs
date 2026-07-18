//! Run the dial/listen handshake over an already-connected
//! [`BinaryDuplex`]. Spawn-free version of the canonical
//! [`frtun_pair::handshake_dial`] / [`frtun_pair::handshake_listen`].
//!
//! Returns the post-handshake `BinaryDuplex` + remote peer name as a
//! tuple — the caller decides what to do with the byte stream. Native
//! consumers wrap it into a tokio-actor `PairStream` via
//! [`frtun_pair::PairStream::spawn`]; wasm consumers spawn-local a
//! `wasm_bindgen_futures::spawn_local` byte-pump.

use crate::protocol::{ClientFrame, ServerFrame};
use crate::stream::BinaryDuplex;
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
/// expects `Dialed { peer }` back, and returns the post-handshake byte
/// stream + remote peer name. Caller wires whatever post-handshake
/// read/write loop suits the target (tokio actor on native;
/// `spawn_local` pump on wasm).
pub async fn handshake_dial<D>(
    mut inner: D,
    self_peer:   &str,
    remote_peer: &str,
) -> Result<(D, String), HandshakeError>
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
        ServerFrame::Dialed { peer } => Ok((inner, peer)),
        ServerFrame::Error { code, msg } => Err(HandshakeError::BridgeRejected { code, msg }),
        other => Err(HandshakeError::UnexpectedFrame(other)),
    }
}

/// Run the **listen** handshake. Sends `Listen { peer }`, expects
/// `Ready` then later `Incoming { peer }`, and returns the
/// post-handshake byte stream + dialing peer's name.
pub async fn handshake_listen<D>(
    inner:     D,
    self_peer: &str,
) -> Result<(D, String), HandshakeError>
where
    D: BinaryDuplex,
{
    handshake_listen_with_token(inner, self_peer, None).await
}

/// Variant of [`handshake_listen`] that ALSO registers an FCM device
/// token with the bridge in the same `Listen` frame, so a future Dial
/// for this peer name CAN wake the device via `fcm-wake` if it isn't
/// listening at that moment. Pass `None` for the plain pre-FCM shape
/// — byte-identical to the legacy wire form.
pub async fn handshake_listen_with_token<D>(
    mut inner: D,
    self_peer: &str,
    fcm_token: Option<String>,
) -> Result<(D, String), HandshakeError>
where
    D: BinaryDuplex,
{
    inner.send_text(
        ClientFrame::Listen { peer: self_peer.to_string(), fcm_token }.to_json()
    ).await?;

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
        ServerFrame::Incoming { peer } => Ok((inner, peer)),
        ServerFrame::Error { code, msg } => Err(HandshakeError::BridgeRejected { code, msg }),
        other => Err(HandshakeError::UnexpectedFrame(other)),
    }
}
