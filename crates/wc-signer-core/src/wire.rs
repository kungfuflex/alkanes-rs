//! Plaintext envelopes that get encrypted before hitting the bridge.
//!
//! Byte-identical to `subfrost-wallet-wc::wire` in the SUBFROST mobile
//! workspace (vc=419) — the dapp serialises one of these as JSON,
//! encrypts with symKey, ships `{ciphertextB64, nonceB64}` over the
//! `/v1/pair` rendezvous, the phone decrypts + acts + ships back a
//! `Result`/`Error`/`Accounts` envelope through the same stream.
//!
//! The on-the-wire envelope is the minimal camelCase shape — the legacy
//! `RequestEnvelope { origin, request_id, ... }` from the OLD wc-relay
//! protocol is GONE; the new transport only carries `{ciphertextB64,
//! nonceB64}`.

use serde::{Deserialize, Serialize};

/// Tagged-union of every plaintext shape that crosses the wire.
/// Mirrors the TS sender + the SUBFROST mobile listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Plaintext {
    /// dapp → phone: sign the supplied PSBT (hex-encoded).
    SignPsbt {
        psbt_hex: String,
        addresses: Vec<String>,
        request_id: String,
        origin: String,
    },
    /// dapp → phone: sign a bitcoin-signed-message / BIP-322.
    SignMessage {
        message: String,
        address: String,
        request_id: String,
        origin: String,
    },
    /// dapp → phone: ask for the current address list.
    GetAccounts {
        request_id: String,
        origin: String,
    },
    /// phone → dapp: opaque string result (signed PSBT hex, signature,
    /// etc.) keyed by `request_id`.
    Result {
        request_id: String,
        result: String,
    },
    /// phone → dapp: wallet rejected with a stable code.
    Error {
        request_id: String,
        /// `user_rejected` | `permission_denied` | `internal` | …
        code: String,
        message: String,
    },
    /// phone → dapp: ordered list of addresses.
    Accounts {
        request_id: String,
        addresses: Vec<String>,
    },
}

/// Minimal envelope shape the new protocol sends over the bridge —
/// matches the TS sender's `JSON.stringify({ciphertextB64, nonceB64})`
/// byte-for-byte. Keep these as `camelCase` for that exact match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireEnvelope {
    #[serde(rename = "ciphertextB64")]
    pub ciphertext_b64: String,
    #[serde(rename = "nonceB64")]
    pub nonce_b64: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_sign_psbt_round_trip() {
        let p = Plaintext::SignPsbt {
            psbt_hex: "deadbeef".into(),
            addresses: vec!["bc1qaddr".into()],
            request_id: "req-1".into(),
            origin: "cli://test".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        // Tag + snake_case shape must match what the TS sender ships.
        assert!(s.contains("\"type\":\"sign_psbt\""));
        assert!(s.contains("\"request_id\":\"req-1\""));
        let back: Plaintext = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn envelope_wire_shape_is_camel_case() {
        let env = WireEnvelope {
            ciphertext_b64: "AAAA".into(),
            nonce_b64: "BBBB".into(),
        };
        let s = serde_json::to_string(&env).unwrap();
        // EXACT byte-match against the TS sender's
        // `JSON.stringify({ciphertextB64, nonceB64})` (alphabetical
        // ordering is what serde defaults to but we don't rely on
        // ordering for the on-the-wire contract — both fields present).
        assert!(s.contains("\"ciphertextB64\":\"AAAA\""));
        assert!(s.contains("\"nonceB64\":\"BBBB\""));
        let back: WireEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env, back);
    }

    #[test]
    fn result_round_trip() {
        let p = Plaintext::Result {
            request_id: "req-2".into(),
            result: "signed-hex".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"type\":\"result\""));
        let back: Plaintext = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn error_round_trip() {
        let p = Plaintext::Error {
            request_id: "req-3".into(),
            code: "user_rejected".into(),
            message: "nope".into(),
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Plaintext = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn accounts_round_trip() {
        let p = Plaintext::Accounts {
            request_id: "req-4".into(),
            addresses: vec!["bc1qa".into(), "bc1qb".into()],
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Plaintext = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
