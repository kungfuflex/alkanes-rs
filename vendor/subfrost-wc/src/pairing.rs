//! Parse + accept the QR pairing URI.
//!
//! URI shape:
//!
//!   subfrost://wc/<topic-uuid>?key=<base64url-x25519-pub>&relay=wss://...&origin=https://...
//!
//! `relay` is optional (defaults to the build-time
//! `SUBFROST_WC_RELAY_URL` or `wss://wc.subfrost.io/`). `origin` is
//! the webapp origin the user is pairing with — surfaced in
//! PairingScreen so the mobile user can authorize per-site.

use thiserror::Error;
use url::Url;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::crypto::{self, KEY_LEN};

#[derive(Debug, Error)]
pub enum PairingError {
    #[error("not a subfrost://wc/ URI")]
    BadScheme,
    #[error("missing topic in path")]
    MissingTopic,
    #[error("missing key= query param")]
    MissingKey,
    #[error("invalid key: {0}")]
    BadKey(String),
    #[error("malformed url: {0}")]
    BadUrl(String),
    #[error("crypto: {0}")]
    Crypto(#[from] crypto::CryptoError),
}

/// Pre-acceptance pairing — we have the webapp's pubkey + topic + a
/// freshly-minted mobile keypair + the derived symmetric key, but we
/// haven't told the relay we accept yet. The Compose UX shows the
/// PairingScreen using this struct and then commits via
/// `accept_pairing` (out-of-crate, in `subfrost-mobile-ffi`).
#[derive(Clone)]
pub struct PendingPairing {
    pub topic:       String,
    pub origin:      Option<String>,
    pub relay_url:   String,
    pub webapp_pub:  PublicKey,
    pub mobile_priv: StaticSecret,
    pub mobile_pub:  PublicKey,
    pub sym_key:     [u8; KEY_LEN],
}

// Production wc-relay: Cloud Run in lithomantic-heaven-bestary,
// fronted by a Cloudflare Worker that rewrites Host so the run.app
// service accepts the request. DNS: wc.subfrost.io → CF Worker.
const DEFAULT_RELAY: &str = "wss://wc.subfrost.io/";

pub fn parse_pairing_uri(uri: &str) -> Result<PendingPairing, PairingError> {
    let parsed = Url::parse(uri.trim()).map_err(|e| PairingError::BadUrl(e.to_string()))?;
    if parsed.scheme() != "subfrost" {
        return Err(PairingError::BadScheme);
    }
    if parsed.host_str() != Some("wc") {
        return Err(PairingError::BadScheme);
    }
    let topic = parsed.path().trim_start_matches('/').to_string();
    if topic.is_empty() {
        return Err(PairingError::MissingTopic);
    }

    let mut webapp_pub_b64: Option<String> = None;
    let mut relay_url:      Option<String> = None;
    let mut origin:         Option<String> = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "key"    => webapp_pub_b64 = Some(v.into_owned()),
            "relay"  => relay_url      = Some(v.into_owned()),
            "origin" => origin         = Some(v.into_owned()),
            _ => {}
        }
    }
    let webapp_pub_b64 = webapp_pub_b64.ok_or(PairingError::MissingKey)?;
    let webapp_pub = crypto::pub_from_b64url(&webapp_pub_b64)
        .map_err(|e| PairingError::BadKey(format!("{e}")))?;

    let (mobile_priv, mobile_pub) = crypto::gen_keypair();
    let sym_key = crypto::ecdh_derive(&mobile_priv, &webapp_pub, &topic)?;

    Ok(PendingPairing {
        topic,
        origin,
        relay_url: relay_url.unwrap_or_else(|| DEFAULT_RELAY.to_string()),
        webapp_pub,
        mobile_priv,
        mobile_pub,
        sym_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    #[test]
    fn parse_round_trip() {
        let (priv_w, pub_w) = crypto::gen_keypair();
        let topic = uuid::Uuid::new_v4().to_string();
        let key_b64 = crypto::pub_to_b64url(&pub_w);
        let uri = format!(
            "subfrost://wc/{topic}?key={key_b64}&origin=https%3A%2F%2Fapp.subfrost.io",
        );
        let pending = parse_pairing_uri(&uri).unwrap();
        assert_eq!(pending.topic, topic);
        assert_eq!(pending.origin.as_deref(), Some("https://app.subfrost.io"));
        // Webapp side derives the same key.
        let key_b = crypto::ecdh_derive(&priv_w, &pending.mobile_pub, &topic).unwrap();
        assert_eq!(pending.sym_key, key_b);
    }

    #[test]
    fn reject_wrong_scheme() {
        let r = parse_pairing_uri("https://wc.example/abc?key=foo");
        assert!(matches!(r, Err(PairingError::BadScheme)));
    }
}
