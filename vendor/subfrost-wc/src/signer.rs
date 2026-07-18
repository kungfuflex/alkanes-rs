//! High-level WalletConnect signer.
//!
//! Combines:
//!   * a pairing URI + ECDH keypair (from [`crate::crypto`] +
//!     [`crate::pairing`]),
//!   * a relay client (from [`crate::relay`]),
//!   * the Plaintext wire enum (from [`crate::wire`]),
//!
//! into a single object exposing three signing methods:
//!
//!   * `sign_psbt(psbt_hex, addresses) -> signed_psbt_hex`
//!   * `sign_message(message, address) -> signature_b64`
//!   * `get_accounts() -> Vec<address>`
//!
//! The signer takes care of:
//!   * encrypting the request plaintext with the session symKey,
//!   * publishing it via the relay,
//!   * decrypting the wallet's response,
//!   * unwrapping `Plaintext::Result` / `Plaintext::Error` /
//!     `Plaintext::Accounts` into a `Result`.
//!
//! Construction is two-step so callers can show a "pair your phone"
//! prompt between the `start_pairing` and `complete_pairing` calls.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use thiserror::Error;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{
    crypto::{decrypt, ecdh_derive, encrypt, gen_keypair, CryptoError, KEY_LEN, NONCE_LEN},
    pairing::{parse_pairing_uri, PairingError, PendingPairing},
    relay::{DappRelay, RelayConfig, RelayError},
    wire::{Plaintext, RequestEnvelope},
};

#[derive(Debug, Error)]
pub enum SignerError {
    #[error("pairing parse: {0}")]
    Pairing(#[from] PairingError),
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
    #[error("relay: {0}")]
    Relay(#[from] RelayError),
    #[error("wallet rejected: {code} — {message}")]
    UserRejected { code: String, message: String },
    #[error("unexpected response variant for {expected}: {got}")]
    UnexpectedResponse {
        expected: &'static str,
        got: &'static str,
    },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not paired yet")]
    NotPaired,
}

/// Pairing-in-progress. Hold this between `start_pairing` (which prints
/// the URI for the user to scan) and `complete_pairing` (which blocks
/// until the wallet accepts).
pub struct PairingInProgress {
    pub uri: String,
    pub topic: String,
    own_priv: StaticSecret,
    relay: DappRelay,
    /// Relay endpoint (e.g. `wss://wc.subfrost.io/`). Kept on the
    /// pairing handle so a successful `complete()` can stash it on the
    /// resulting signer for later session restore.
    relay_url: String,
    /// The optional 6-char code printed by the dapp side for the user
    /// to type into the wallet — mixed into HKDF along with the topic.
    code: Option<String>,
    /// "https://alkanes-cli.local" or similar — surfaced to the wallet
    /// in every plaintext so it can show "you're signing for X".
    origin: String,
}

/// A live WalletConnect session ready to sign.
pub struct WalletConnectSigner {
    sym_key: [u8; KEY_LEN],
    relay: DappRelay,
    topic: String,
    origin: String,
    relay_url: String,
    accounts: Vec<String>,
}

/// Serializable on-disk representation of a paired session. The symKey is
/// the only sensitive field — store the resulting file with mode 0600 (or
/// the platform equivalent). Compromise of the symKey alone is not catastrophic
/// because the mobile wallet still has to approve every sign request, but
/// it would let an attacker eavesdrop on subsequent sign traffic.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedSession {
    pub topic: String,
    pub relay_url: String,
    pub origin: String,
    /// 32-byte ChaCha20-Poly1305 key, base64-encoded.
    pub sym_key_b64: String,
    pub accounts: Vec<String>,
}

impl core::fmt::Debug for WalletConnectSigner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WalletConnectSigner")
            .field("topic", &self.topic)
            .field("origin", &self.origin)
            .finish()
    }
}

/// Build a pairing URI fresh. Generates a new keypair and pairing code,
/// connects to the relay, sends `init`, waits for `init_ack`. The
/// returned URI is what the user scans/types into their mobile wallet.
///
/// `relay_url` should be `wss://wc.subfrost.io/` for production.
pub async fn start_pairing(
    relay_url: impl Into<String>,
    origin: impl Into<String>,
) -> Result<PairingInProgress, SignerError> {
    let relay_url = relay_url.into();
    let origin = origin.into();
    let topic = Uuid::new_v4().to_string();
    let (own_priv, own_pub) = gen_keypair();
    let own_pub_b64 = B64.encode(own_pub.as_bytes());

    // 6-character code (uppercase alphanumeric, no ambiguous chars).
    let code = pairing_code();

    let uri = format!(
        "subfrost://wc/{topic}?key={key}&mode=cli&relay={relay}&origin={origin_enc}&code={code}",
        topic = topic,
        key = url_param_encode(&own_pub_b64),
        relay = url_param_encode(&relay_url),
        origin_enc = url_param_encode(&origin),
        code = code,
    );

    let relay = DappRelay::new(RelayConfig {
        relay_url: relay_url.clone(),
        topic: topic.clone(),
        webapp_pub_b64: own_pub_b64.clone(),
    });
    relay.open().await?;

    Ok(PairingInProgress {
        uri,
        topic,
        own_priv,
        relay,
        relay_url,
        code: Some(code),
        origin,
    })
}

/// Restore from a pairing URI someone else printed (uncommon for the
/// CLI; typical for QR scanners on mobile). The CLI would normally use
/// [`start_pairing`] instead.
pub async fn join_pairing(uri: &str) -> Result<PairingInProgress, SignerError> {
    let pending: PendingPairing = parse_pairing_uri(uri)?;
    let (own_priv, own_pub) = gen_keypair();
    let own_pub_b64 = B64.encode(own_pub.as_bytes());
    let relay = DappRelay::new(RelayConfig {
        relay_url: pending.relay_url.clone(),
        topic: pending.topic.clone(),
        webapp_pub_b64: own_pub_b64,
    });
    relay.open().await?;
    let relay_url = pending.relay_url.clone();
    Ok(PairingInProgress {
        uri: uri.to_string(),
        topic: pending.topic,
        own_priv,
        relay,
        relay_url,
        // The pairing-URI parser doesn't extract `code` today (it's a
        // dapp-side-only artifact for CLI mode), so we leave it None
        // here. join_pairing is rare; CLI flow prefers start_pairing.
        code: None,
        origin: pending.origin.unwrap_or_else(|| "alkanes-cli".into()),
    })
}

impl PairingInProgress {
    /// Block until the wallet pairs. Returns a ready-to-sign session.
    /// `timeout` is how long to wait for the user to scan + accept.
    pub async fn complete(self, timeout: Duration) -> Result<WalletConnectSigner, SignerError> {
        let mobile_pub_b64 = self.relay.await_accepted(timeout).await?;
        let mobile_pub_bytes = B64
            .decode(&mobile_pub_b64)
            .map_err(|e| SignerError::Crypto(CryptoError::BadPubLen(format!("{e}").len())))?;
        if mobile_pub_bytes.len() != 32 {
            return Err(SignerError::Crypto(CryptoError::BadPubLen(
                mobile_pub_bytes.len(),
            )));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&mobile_pub_bytes);
        let mobile_pub = PublicKey::from(arr);

        // HKDF info binds the symKey to both the topic AND the optional
        // pairing code, so a mismatch (user typed wrong code) produces
        // a different key on each side → all subsequent decrypts fail.
        let hkdf_info = match &self.code {
            Some(c) => format!("{}:{}", self.topic, c),
            None => self.topic.clone(),
        };
        let sym_key = ecdh_derive(&self.own_priv, &mobile_pub, &hkdf_info)?;
        // We can't pull relay_url out of DappRelay (private field), so
        // re-cache it on the signer for later persistence.
        let relay_url = self.relay_url.clone();
        Ok(WalletConnectSigner {
            sym_key,
            relay: self.relay,
            topic: self.topic,
            origin: self.origin,
            relay_url,
            accounts: Vec::new(),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
    pub fn topic(&self) -> &str {
        &self.topic
    }
}

impl WalletConnectSigner {
    /// Sign a PSBT (hex-encoded). `addresses` is the set of taproot/
    /// segwit addresses whose inputs the wallet should sign for. Returns
    /// the fully-signed PSBT as hex.
    pub async fn sign_psbt(
        &self,
        psbt_hex: &str,
        addresses: Vec<String>,
    ) -> Result<String, SignerError> {
        let request_id = Uuid::new_v4().to_string();
        let req = Plaintext::SignPsbt {
            psbt_hex: psbt_hex.to_string(),
            addresses,
            request_id: request_id.clone(),
            origin: self.origin.clone(),
        };
        match self.send(req, &request_id).await? {
            Plaintext::Result { result, .. } => Ok(result),
            other => Err(unexpected("Result(signed_psbt_hex)", &other)),
        }
    }

    pub async fn sign_message(
        &self,
        message: &str,
        address: &str,
    ) -> Result<String, SignerError> {
        let request_id = Uuid::new_v4().to_string();
        let req = Plaintext::SignMessage {
            message: message.to_string(),
            address: address.to_string(),
            request_id: request_id.clone(),
            origin: self.origin.clone(),
        };
        match self.send(req, &request_id).await? {
            Plaintext::Result { result, .. } => Ok(result),
            other => Err(unexpected("Result(signature_b64)", &other)),
        }
    }

    pub async fn get_accounts(&self) -> Result<Vec<String>, SignerError> {
        let request_id = Uuid::new_v4().to_string();
        let req = Plaintext::GetAccounts {
            request_id: request_id.clone(),
            origin: self.origin.clone(),
        };
        match self.send(req, &request_id).await? {
            Plaintext::Accounts { addresses, .. } => Ok(addresses),
            other => Err(unexpected("Accounts", &other)),
        }
    }

    async fn send(&self, plaintext: Plaintext, request_id: &str) -> Result<Plaintext, SignerError> {
        let plaintext_bytes = serde_json::to_vec(&plaintext)?;
        let (ciphertext, nonce) = encrypt(&self.sym_key, &plaintext_bytes)?;
        let env = RequestEnvelope {
            ciphertext: B64.encode(&ciphertext),
            nonce: B64.encode(nonce),
            origin: self.origin.clone(),
            request_id: request_id.to_string(),
        };
        let resp = self.relay.send_request(env, None).await?;
        let resp_ciphertext = B64
            .decode(&resp.ciphertext)
            .map_err(|e| SignerError::Json(serde::de::Error::custom(format!("b64 ct: {e}"))))?;
        let nonce_bytes = B64
            .decode(&resp.nonce)
            .map_err(|e| SignerError::Json(serde::de::Error::custom(format!("b64 nonce: {e}"))))?;
        if nonce_bytes.len() != NONCE_LEN {
            return Err(SignerError::Crypto(CryptoError::BadNonceLen(
                nonce_bytes.len(),
            )));
        }
        let mut nonce_arr = [0u8; NONCE_LEN];
        nonce_arr.copy_from_slice(&nonce_bytes);
        let decrypted = decrypt(&self.sym_key, &resp_ciphertext, &nonce_arr)?;
        let plaintext: Plaintext = serde_json::from_slice(&decrypted)?;
        // Convert Error variant into SignerError eagerly.
        if let Plaintext::Error { code, message, .. } = &plaintext {
            return Err(SignerError::UserRejected {
                code: code.clone(),
                message: message.clone(),
            });
        }
        Ok(plaintext)
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub fn accounts(&self) -> &[String] {
        &self.accounts
    }

    /// Cache the account list on the signer. Typically called once right
    /// after pairing — the result is stored in the persisted-session
    /// blob so future `--use-walletconnect` invocations don't have to
    /// round-trip just to discover which addresses to spend from.
    pub fn set_accounts(&mut self, accounts: Vec<String>) {
        self.accounts = accounts;
    }

    /// Serialize to disk-storable form. The symKey is base64'd; callers
    /// MUST write the resulting JSON with 0600 permissions (or whatever
    /// "owner-only readable" looks like on the host platform).
    pub fn to_persisted(&self) -> PersistedSession {
        PersistedSession {
            topic: self.topic.clone(),
            relay_url: self.relay_url.clone(),
            origin: self.origin.clone(),
            sym_key_b64: B64.encode(self.sym_key),
            accounts: self.accounts.clone(),
        }
    }

    /// Re-attach to a previously-paired session. Opens a fresh WSS
    /// (`subscribe`-style, no new `init`/key derivation), and returns
    /// a signer ready to use the persisted symKey.
    ///
    /// Fails if the relay rejects the subscribe (typically because the
    /// session expired on the relay side — pair again).
    pub async fn restore(session: PersistedSession) -> Result<Self, SignerError> {
        let sym_key_bytes = B64
            .decode(&session.sym_key_b64)
            .map_err(|e| SignerError::Crypto(CryptoError::BadPubLen(format!("{e}").len())))?;
        if sym_key_bytes.len() != KEY_LEN {
            return Err(SignerError::Crypto(CryptoError::BadPubLen(
                sym_key_bytes.len(),
            )));
        }
        let mut sym_key = [0u8; KEY_LEN];
        sym_key.copy_from_slice(&sym_key_bytes);

        // We don't have the dapp pub anymore — but reconnect just sends
        // `subscribe`, no key exchange.
        let relay = DappRelay::new(RelayConfig {
            relay_url: session.relay_url.clone(),
            topic: session.topic.clone(),
            webapp_pub_b64: String::new(),
        });
        relay.reconnect().await?;

        Ok(Self {
            sym_key,
            relay,
            topic: session.topic,
            origin: session.origin,
            relay_url: session.relay_url,
            accounts: session.accounts,
        })
    }
}

fn unexpected(expected: &'static str, got: &Plaintext) -> SignerError {
    let got_name = match got {
        Plaintext::SignPsbt { .. } => "SignPsbt",
        Plaintext::SignMessage { .. } => "SignMessage",
        Plaintext::GetAccounts { .. } => "GetAccounts",
        Plaintext::Result { .. } => "Result",
        Plaintext::Error { .. } => "Error",
        Plaintext::Accounts { .. } => "Accounts",
    };
    SignerError::UnexpectedResponse {
        expected,
        got: got_name,
    }
}

fn pairing_code() -> String {
    // 6 chars from a 32-symbol alphabet, no ambiguous chars (0/O, 1/I).
    const ALPHA: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut out = String::with_capacity(6);
    let mut buf = [0u8; 6];
    rand_core::OsRng.fill_bytes(&mut buf);
    for b in buf.iter() {
        out.push(ALPHA[(*b as usize) % ALPHA.len()] as char);
    }
    out
}

use rand_core::RngCore;

fn url_param_encode(s: &str) -> String {
    // Minimal URL-encoding for the param-value characters we actually
    // care about (=, &, ?, #, space, slash, %). For full URL escaping
    // any caller can replace this with `percent-encoding` later.
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '&' => out.push_str("%26"),
            '=' => out.push_str("%3D"),
            '?' => out.push_str("%3F"),
            '#' => out.push_str("%23"),
            ' ' => out.push_str("%20"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

    #[test]
    fn persisted_session_roundtrips_through_json() {
        let original = PersistedSession {
            topic: "abc-123".to_string(),
            relay_url: "wss://wc.subfrost.io/".to_string(),
            origin: "alkanes-cli".to_string(),
            sym_key_b64: B64.encode([7u8; 32]),
            accounts: vec![
                "bc1p…aaa".to_string(),
                "bc1q…bbb".to_string(),
            ],
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: PersistedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.topic, original.topic);
        assert_eq!(parsed.relay_url, original.relay_url);
        assert_eq!(parsed.sym_key_b64, original.sym_key_b64);
        assert_eq!(parsed.accounts, original.accounts);
    }

    #[test]
    fn restore_rejects_bad_symkey_length() {
        // PersistedSession with a 16-byte symKey (not 32) — should error.
        let bad = PersistedSession {
            topic: "abc".to_string(),
            relay_url: "wss://nowhere.example/".to_string(),
            origin: "test".to_string(),
            sym_key_b64: B64.encode([1u8; 16]),
            accounts: vec![],
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let r = rt.block_on(async move { WalletConnectSigner::restore(bad).await });
        match r {
            Err(SignerError::Crypto(CryptoError::BadPubLen(_))) => {}
            Err(other) => {
                // Connection failure is also acceptable (we point at a
                // bogus URL); we just want to make sure it errors instead
                // of silently using a truncated key.
                eprintln!("got expected error: {other}");
            }
            Ok(_) => panic!("should have errored on bad symkey length"),
        }
    }

    #[test]
    fn pairing_code_shape() {
        let c = pairing_code();
        assert_eq!(c.len(), 6);
        assert!(c.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn url_param_encode_basic() {
        assert_eq!(url_param_encode("a=b&c=d"), "a%3Db%26c%3Dd");
        assert_eq!(url_param_encode("plain"), "plain");
        assert_eq!(url_param_encode("a/b"), "a/b");
    }
}
