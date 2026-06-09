//! Pair URI build + parse for the new SUBFROST mobile WC protocol.
//!
//! URI shape (matches `~/subfrost-mobile/ts-sdk/src/cli.ts::cmdPair`):
//!
//!   subfrost://wc/<cli-bech32-peer>
//!     ?key=<dapp-x25519-pub-b64url>
//!     &code=<6-char-pairing-code>
//!     &bridge=<wss://wss-tls.subfrost.io/v1/pair>
//!     &origin=<...>
//!     &mode=cli
//!
//! The dapp/CLI mints the URI, prints it for the user, then `listen`s
//! on `cli-bech32-peer` on the bridge. The phone scans/pastes, dials the
//! CLI's peer, ECDH-derives the symKey (with `phone_peer:code` mixed
//! into HKDF info), and replies. After that, every request is one
//! binary frame on the same `/v1/pair` stream.

use rand::{rngs::OsRng, RngCore};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum PairingError {
    #[error("not a subfrost://wc/... URI")]
    BadScheme,
    #[error("missing CLI peer name in path")]
    MissingPeer,
    #[error("missing required query param: {0}")]
    MissingParam(&'static str),
    #[error("malformed URI: {0}")]
    BadUrl(String),
}

/// Build the deeplink URI the dapp shows the user.
///
/// `mode` is the wallet-side prompt mode — `"cli"` for headless tools
/// (the wallet shows the pairing code prompt) and `"webapp"` when the
/// dapp injected it via a connect button.
pub fn build_pair_uri(
    cli_peer: &str,
    dapp_pub_b64: &str,
    pairing_code: &str,
    bridge_url: &str,
    origin: &str,
    mode: &str,
) -> String {
    let mut u = String::from("subfrost://wc/");
    u.push_str(&url_encode(cli_peer));
    u.push_str("?key=");
    u.push_str(&url_encode(dapp_pub_b64));
    u.push_str("&code=");
    u.push_str(&url_encode(pairing_code));
    u.push_str("&bridge=");
    u.push_str(&url_encode(bridge_url));
    u.push_str("&origin=");
    u.push_str(&url_encode(origin));
    u.push_str("&mode=");
    u.push_str(&url_encode(mode));
    u
}

/// Parse a `subfrost://wc/<peer>?...` URI into its components. The
/// phone-side parser uses the same shape (mobile FFI's `pair_cli.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPairUri {
    pub cli_peer: String,
    pub dapp_pub_b64: String,
    pub pairing_code: String,
    pub bridge_url: String,
    pub origin: String,
    pub mode: String,
}

pub fn parse_pair_uri(uri: &str) -> Result<ParsedPairUri, PairingError> {
    let parsed = Url::parse(uri.trim()).map_err(|e| PairingError::BadUrl(e.to_string()))?;
    if parsed.scheme() != "subfrost" {
        return Err(PairingError::BadScheme);
    }
    if parsed.host_str() != Some("wc") {
        return Err(PairingError::BadScheme);
    }
    let cli_peer = parsed.path().trim_start_matches('/').to_string();
    if cli_peer.is_empty() {
        return Err(PairingError::MissingPeer);
    }
    let mut key: Option<String> = None;
    let mut code: Option<String> = None;
    let mut bridge: Option<String> = None;
    let mut origin: Option<String> = None;
    let mut mode: Option<String> = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "key" => key = Some(v.into_owned()),
            "code" => code = Some(v.into_owned()),
            "bridge" => bridge = Some(v.into_owned()),
            "origin" => origin = Some(v.into_owned()),
            "mode" => mode = Some(v.into_owned()),
            _ => {}
        }
    }
    Ok(ParsedPairUri {
        cli_peer,
        dapp_pub_b64: key.ok_or(PairingError::MissingParam("key"))?,
        pairing_code: code.ok_or(PairingError::MissingParam("code"))?,
        bridge_url: bridge.ok_or(PairingError::MissingParam("bridge"))?,
        origin: origin.unwrap_or_default(),
        mode: mode.unwrap_or_else(|| "cli".to_string()),
    })
}

/// Mint a fresh 6-char pairing code from the alphabet
/// `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (no 0/1/O/I confusion). Matches
/// the TS-side `generatePairingCode` distribution.
pub fn generate_pairing_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut out = String::with_capacity(6);
    let mut buf = [0u8; 6];
    OsRng.fill_bytes(&mut buf);
    for b in buf {
        out.push(ALPHABET[(b as usize) % ALPHABET.len()] as char);
    }
    out
}

/// Mint a fresh `frtun1<32-hex>.peer` peer name. The bridge validates
/// by prefix + suffix + min length only, so any 32-hex body works.
pub fn generate_cli_peer_name() -> String {
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    let hex = hex_encode(&buf);
    format!("frtun1{}.peer", hex)
}

fn hex_encode(b: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &byte in b {
        s.push(HEX[(byte >> 4) as usize] as char);
        s.push(HEX[(byte & 0x0f) as usize] as char);
    }
    s
}

/// Percent-encode for query / path. Conservative — encode anything that
/// isn't an unreserved URL char.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse_round_trip() {
        let uri = build_pair_uri(
            "frtun1abcdef.peer",
            "AAA",
            "XYZW23",
            "wss://wss-tls.subfrost.io/v1/pair",
            "cli://user",
            "cli",
        );
        let p = parse_pair_uri(&uri).unwrap();
        assert_eq!(p.cli_peer, "frtun1abcdef.peer");
        assert_eq!(p.dapp_pub_b64, "AAA");
        assert_eq!(p.pairing_code, "XYZW23");
        assert_eq!(p.bridge_url, "wss://wss-tls.subfrost.io/v1/pair");
        assert_eq!(p.origin, "cli://user");
        assert_eq!(p.mode, "cli");
    }

    #[test]
    fn rejects_wrong_scheme() {
        let r = parse_pair_uri("https://wc.example/abc?key=foo");
        assert!(matches!(r, Err(PairingError::BadScheme)));
    }

    #[test]
    fn rejects_missing_key() {
        let r = parse_pair_uri("subfrost://wc/peer?code=ABCDEF&bridge=wss://x&origin=o&mode=cli");
        assert!(matches!(r, Err(PairingError::MissingParam("key"))));
    }

    #[test]
    fn pairing_code_is_six_safe_chars() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 6);
        for c in code.chars() {
            assert!(!matches!(c, '0' | '1' | 'O' | 'I'),
                "confusable char in pairing code: {c}");
        }
    }

    #[test]
    fn cli_peer_name_shape() {
        let p = generate_cli_peer_name();
        assert!(p.starts_with("frtun1"));
        assert!(p.ends_with(".peer"));
        // 6 prefix + 32 hex + 5 suffix
        assert_eq!(p.len(), 6 + 32 + 5);
    }
}
