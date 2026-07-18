//! Plaintext envelopes that get encrypted before hitting the relay.
//!
//! Webapp side serializes one of these as JSON, encrypts with symKey,
//! POSTs `{ciphertext, nonce}` to wc-relay. Mobile side fetches +
//! decrypts, matches the type, decides whether the requested
//! permission is in scope, prompts the user.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Plaintext {
    /// Sign a PSBT. `psbt_hex` is the unsigned PSBT (hex-encoded);
    /// the mobile signs locally and returns the signed hex via
    /// `Plaintext::SignPsbtResult`.
    SignPsbt {
        psbt_hex:    String,
        addresses:   Vec<String>,
        request_id:  String,
        origin:      String,
    },
    /// Sign an arbitrary message (BIP322 / Bitcoin Signed Message).
    SignMessage {
        message:     String,
        address:     String,
        request_id:  String,
        origin:      String,
    },
    /// Return the wallet's currently-known accounts. Cheap and used
    /// for adapter `getAccounts()` calls.
    GetAccounts {
        request_id:  String,
        origin:      String,
    },
    /// Response variants — mobile → webapp.
    Result  {
        request_id:  String,
        result:      String,
    },
    Error {
        request_id:  String,
        code:        String,    // "user_rejected" | "permission_denied" | "internal"
        message:     String,
    },
    Accounts {
        request_id:  String,
        addresses:   Vec<String>,
    },
}

/// Serialized envelope sent on the wire. ciphertext + nonce are
/// base64url-encoded for JSON-friendliness; the inner blob is
/// raw ChaCha20-Poly1305 output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub ciphertext:  String,
    pub nonce:       String,
    pub origin:      String,
    pub request_id:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub ciphertext:  String,
    pub nonce:       String,
}
