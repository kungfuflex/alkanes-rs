//! Bridge control protocol.
//!
//! Each peer opens a single WebSocket to the pair bridge, sends one
//! request frame (`Listen` or `Dial`), and receives one response
//! frame (`Ready`, `Dialed`, `Incoming`, or `Error`). After the
//! response, the WebSocket carries raw binary frames in both
//! directions — the bridge forwards them between the two peers'
//! WebSockets verbatim. There is no further control plane after the
//! handshake; multiplexing, encryption, and framing of the payload
//! are the responsibility of the application on top.
//!
//! All frames are JSON, sent as a single WebSocket Text frame each.
//! JSON was chosen over a custom binary format because:
//!
//!   * the handshake exchanges exactly one frame in each direction
//!     before becoming a raw byte stream, so the framing cost is
//!     negligible
//!   * the bridge does not need a code-generator step; a Node.js
//!     bridge would consume the same shape directly
//!   * field evolution stays trivial (add fields, ignore unknowns)

use serde::{Deserialize, Serialize};

/// Frames a peer sends TO the bridge during the handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientFrame {
    /// "I am `peer` — wait for somebody to dial me."
    Listen { peer: String },
    /// "Find `peer` and connect me to them. I am `self_peer`."
    Dial   { peer: String, self_peer: String },
}

/// Frames the bridge sends back during the handshake.
///
/// After a `Ready`, `Dialed`, or `Incoming`, the WebSocket switches
/// to raw binary forwarding — every subsequent frame is forwarded
/// verbatim to the paired peer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerFrame {
    /// `Listen` accepted, waiting for an inbound dial.
    Ready,
    /// `Dial` succeeded; we're now bridged to `peer`.
    Dialed   { peer: String },
    /// An inbound dial arrived; we're now bridged to `peer`.
    Incoming { peer: String },
    /// Handshake failed. `code` is machine-readable, `msg` is for
    /// humans.
    Error    { code: String, msg: String },
}

impl ClientFrame {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ClientFrame serialises")
    }
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

impl ServerFrame {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ServerFrame serialises")
    }
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

/// Stable error codes the bridge returns. Clients should switch on
/// these and surface the matching localised message; `msg` is for
/// developer logs only.
pub mod codes {
    pub const PEER_NOT_FOUND:    &str = "peer_not_found";
    pub const PEER_BUSY:         &str = "peer_busy";
    pub const BAD_PEER_NAME:     &str = "bad_peer_name";
    pub const BAD_FRAME:         &str = "bad_frame";
    pub const INTERNAL:          &str = "internal";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_round_trips() {
        let f = ClientFrame::Listen { peer: "frtun1abc.peer".into() };
        let s = f.to_json();
        assert_eq!(s, r#"{"op":"listen","peer":"frtun1abc.peer"}"#);
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn dial_round_trips() {
        let f = ClientFrame::Dial {
            peer:      "frtun1bob.peer".into(),
            self_peer: "frtun1alice.peer".into(),
        };
        let s = f.to_json();
        // Field order matches the struct declaration so the on-the-wire
        // shape is stable across rustc versions.
        assert!(s.contains(r#""op":"dial""#));
        assert!(s.contains(r#""peer":"frtun1bob.peer""#));
        assert!(s.contains(r#""self_peer":"frtun1alice.peer""#));
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn server_frames_round_trip() {
        let cases = vec![
            ServerFrame::Ready,
            ServerFrame::Dialed   { peer: "frtun1x.peer".into() },
            ServerFrame::Incoming { peer: "frtun1y.peer".into() },
            ServerFrame::Error    { code: codes::PEER_NOT_FOUND.into(), msg: "no such peer".into() },
        ];
        for f in cases {
            let s = f.to_json();
            assert_eq!(ServerFrame::from_json(&s).unwrap(), f);
        }
    }

    #[test]
    fn unknown_op_is_an_error() {
        assert!(ClientFrame::from_json(r#"{"op":"yodel","peer":"x"}"#).is_err());
    }

    #[test]
    fn ready_serialises_minimally() {
        // {"event":"ready"} — no payload fields.
        assert_eq!(ServerFrame::Ready.to_json(), r#"{"event":"ready"}"#);
    }
}
