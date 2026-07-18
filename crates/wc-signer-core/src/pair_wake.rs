//! Tiny helper around the `/v1/pair-wake` HTTP route. The signer's
//! `dial_with_wake` calls `WalletTransport::pair_wake` directly; this
//! module exists so callers (e.g. mobile FFI registering its FCM token
//! shape) have a stable place to hang the request/response wire shapes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairWakeRequest {
    pub peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairWakeResponse {
    pub delivered: bool,
    /// "ok" | "no_kv" | "no_token" | "no_fcm" | "send_err"
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_round_trip() {
        let r = PairWakeRequest { peer: "frtun1abc.peer".into() };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("frtun1abc.peer"));
        let back: PairWakeRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.peer, r.peer);
    }

    #[test]
    fn resp_round_trip() {
        let r = PairWakeResponse { delivered: true, reason: "ok".into() };
        let s = serde_json::to_string(&r).unwrap();
        let back: PairWakeResponse = serde_json::from_str(&s).unwrap();
        assert!(back.delivered);
        assert_eq!(back.reason, "ok");
    }
}
