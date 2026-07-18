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
///
/// **Backward-compatibility note:** `Listen.fcm_token` and the new
/// `Register` variant are both opt-in. Old clients that don't send
/// these fields keep working — the bridge falls through to the legacy
/// "no wake" path when no token is on file. The default field on
/// `Listen` is `None` (via `#[serde(default)]`), so existing JSON of
/// shape `{"op":"listen","peer":"frtun1…"}` parses unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientFrame {
    /// "I am `peer` — wait for somebody to dial me." Optionally
    /// registers a wake token in the same frame so a future cold-state
    /// Dial can wake the device.
    Listen {
        peer: String,
        /// Optional FCM device token. When present + the
        /// `frtun-pair-bridge` is running with `fcm-wake`, the bridge
        /// registers the (peer, token) pair so future Dials to this
        /// peer can fire FCM wake-up if the peer isn't currently
        /// listening. Default `None` (no wake registered).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fcm_token: Option<String>,
    },
    /// "Find `peer` and connect me to them. I am `self_peer`."
    Dial   { peer: String, self_peer: String },
    /// Register/refresh a wake token for `peer` WITHOUT taking a
    /// listener slot. Useful when a mobile wants to register its FCM
    /// token at app launch without committing to a long-lived Listen
    /// (it'll listen on demand when the wake fires). The bridge
    /// responds with `ServerFrame::Registered` and closes the
    /// connection.
    Register {
        peer: String,
        /// FCM device token. `None` here means "deregister".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fcm_token: Option<String>,
    },
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
    /// `Register` accepted. Connection closes immediately after. Only
    /// emitted in response to a `ClientFrame::Register` frame.
    Registered { peer: String },
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
        // With `fcm_token: None`, `skip_serializing_if` drops the
        // field — the wire shape stays byte-identical to the
        // pre-fcm-wake schema. Old clients consume this unchanged.
        let f = ClientFrame::Listen { peer: "frtun1abc.peer".into(), fcm_token: None };
        let s = f.to_json();
        assert_eq!(s, r#"{"op":"listen","peer":"frtun1abc.peer"}"#);
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn listen_with_fcm_token_round_trips() {
        let f = ClientFrame::Listen {
            peer:      "frtun1abc.peer".into(),
            fcm_token: Some("FCM_TOKEN_HERE".into()),
        };
        let s = f.to_json();
        assert!(s.contains(r#""op":"listen""#));
        assert!(s.contains(r#""peer":"frtun1abc.peer""#));
        assert!(s.contains(r#""fcm_token":"FCM_TOKEN_HERE""#));
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn old_listen_json_parses_with_default_none_token() {
        // Pre-fcm-wake clients send `{"op":"listen","peer":"..."}` —
        // serde::default fills `fcm_token` with None.
        let s = r#"{"op":"listen","peer":"frtun1abc.peer"}"#;
        let f = ClientFrame::from_json(s).unwrap();
        match f {
            ClientFrame::Listen { peer, fcm_token } => {
                assert_eq!(peer, "frtun1abc.peer");
                assert!(fcm_token.is_none());
            }
            _ => panic!("expected Listen"),
        }
    }

    #[test]
    fn register_round_trips() {
        let f = ClientFrame::Register {
            peer:      "frtun1abc.peer".into(),
            fcm_token: Some("FCM_TOKEN".into()),
        };
        let s = f.to_json();
        assert!(s.contains(r#""op":"register""#));
        assert!(s.contains(r#""peer":"frtun1abc.peer""#));
        assert!(s.contains(r#""fcm_token":"FCM_TOKEN""#));
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn register_with_none_token_deregisters() {
        let f = ClientFrame::Register {
            peer:      "frtun1abc.peer".into(),
            fcm_token: None,
        };
        let s = f.to_json();
        assert_eq!(s, r#"{"op":"register","peer":"frtun1abc.peer"}"#);
        assert_eq!(ClientFrame::from_json(&s).unwrap(), f);
    }

    #[test]
    fn registered_round_trips() {
        let f = ServerFrame::Registered { peer: "frtun1abc.peer".into() };
        let s = f.to_json();
        assert!(s.contains(r#""event":"registered""#));
        assert_eq!(ServerFrame::from_json(&s).unwrap(), f);
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
            ServerFrame::Dialed     { peer: "frtun1x.peer".into() },
            ServerFrame::Incoming   { peer: "frtun1y.peer".into() },
            ServerFrame::Registered { peer: "frtun1z.peer".into() },
            ServerFrame::Error      { code: codes::PEER_NOT_FOUND.into(), msg: "no such peer".into() },
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
