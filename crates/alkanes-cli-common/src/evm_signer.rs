//! Local EIP-1559 signing and PRIVATE submission for the EVM side of the bridge.
//!
//! # Why this exists rather than shelling out to a wallet
//!
//! The bridge's Ethereum leg is `approve` + `depositAndBridge`. For an
//! interactive user, handing off to MetaMask is right — but an unattended agent
//! or a scripted workflow has no browser, and leaving that gap means the
//! "full circle" is never closed.
//!
//! # The submission path is the point
//!
//! ⚠️ Broadcast goes to `/v4/{apikey}/ethereum-builder`, NOT to a public RPC.
//! That route records the pending broadcast and then blasts
//! `eth_sendRawTransaction` concurrently at a builder set — the transaction
//! never enters the public mempool.
//!
//! This matters specifically for THIS trade. A `depositAndBridge` carries the
//! BTC destination and the conversion split in its calldata, and the Bitcoin
//! leg's `min_dy` becomes visible in an OP_RETURN for as long as it takes to
//! confirm. Broadcasting the EVM leg publicly hands a searcher advance notice of
//! a swap that is about to happen against a shallow pool. The edge config says
//! the same thing in its own words: route order there is load-bearing because
//! `/ethereum` is a substring of `/ethereum-builder`, and getting it wrong is
//! "wrong destination AND a privacy leak".
//!
//! The builder route is submit-only and never signs — so the key stays here,
//! and only signed bytes leave.
//!
//! # Key material
//!
//! Three mechanisms, in descending order of how much you should like them:
//!
//! 1. `--eth-key-file` — a file containing the hex key, ideally mode 0600.
//! 2. `ETHEREUM_PRIVATE_KEY` — environment. Fine for CI and unattended runs;
//!    it is visible to anything that can read the process environment.
//! 3. `--eth-private-key` — argument. **Refused unless you also pass
//!    `--i-know-this-lands-in-shell-history`**, because it does.
//!
//! Whatever the source, the key is zeroized after use, never logged, and never
//! included in an error message.

use anyhow::{anyhow, bail, Result};
use secp256k1::{Message, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};

/// Ethereum mainnet. The vault is deployed here and nowhere else.
pub const CHAIN_ID: u64 = 1;

/// Where signed bytes go. `{}` is the api key.
///
/// ⚠️ Never substitute `/ethereum` here — that is the READ path, and sending a
/// broadcast to it puts the transaction in the public mempool.
pub const BUILDER_PATH: &str = "/v4/{}/ethereum-builder";

/// A loaded key. Zeroized on drop.
pub struct EthKey {
    secret: SecretKey,
    address: [u8; 20],
}

impl Drop for EthKey {
    fn drop(&mut self) {
        // secp256k1::SecretKey zeroizes itself; this is belt-and-braces for the
        // derived material we hold alongside it.
        self.address.fill(0);
    }
}

impl std::fmt::Debug for EthKey {
    /// Deliberately prints the ADDRESS only. A `Debug` that could print key
    /// material is one that eventually ends up in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EthKey({})", self.address_hex())
    }
}

impl EthKey {
    /// Parse a hex private key, with or without `0x`.
    pub fn from_hex(s: &str) -> Result<Self> {
        let t = s.trim();
        let t = t.strip_prefix("0x").unwrap_or(t);
        if t.len() != 64 || !t.bytes().all(|b| b.is_ascii_hexdigit()) {
            // Deliberately does not echo the input — an error message is the
            // easiest place for a key to leak.
            bail!("private key must be 32 bytes of hex (64 characters)");
        }
        let raw = hex::decode(t).map_err(|_| anyhow!("private key is not valid hex"))?;
        let secret = SecretKey::from_slice(&raw)
            .map_err(|_| anyhow!("private key is not a valid secp256k1 scalar"))?;
        let address = derive_address(&secret);
        Ok(Self { secret, address })
    }

    /// Resolve a key from, in order: an explicit file, the environment, or an
    /// inline argument that must be explicitly acknowledged.
    ///
    /// Returns a clear error naming all three mechanisms when none is present —
    /// an agent that cannot find a key should be told how to supply one.
    pub fn resolve(
        key_file: Option<&str>,
        inline: Option<&str>,
        allow_inline: bool,
    ) -> Result<Self> {
        if let Some(path) = key_file {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| anyhow!("could not read --eth-key-file {path}: {e}"))?;
            return Self::from_hex(&raw);
        }
        if let Ok(v) = std::env::var("ETHEREUM_PRIVATE_KEY") {
            if !v.trim().is_empty() {
                return Self::from_hex(&v);
            }
        }
        if let Some(v) = inline {
            if !allow_inline {
                bail!(
                    "--eth-private-key puts key material in your shell history and process \
                     table. Pass --i-know-this-lands-in-shell-history to proceed, or prefer \
                     --eth-key-file or the ETHEREUM_PRIVATE_KEY environment variable."
                );
            }
            return Self::from_hex(v);
        }
        bail!(
            "no Ethereum key. Supply one of: --eth-key-file <path>, \
             ETHEREUM_PRIVATE_KEY=0x…, or --eth-private-key (requires \
             --i-know-this-lands-in-shell-history)."
        )
    }

    pub fn address(&self) -> [u8; 20] {
        self.address
    }

    pub fn address_hex(&self) -> String {
        format!("0x{}", hex::encode(self.address))
    }

    /// Sign an EIP-1559 transaction and return the raw bytes to broadcast.
    pub fn sign_1559(&self, tx: &Eip1559Tx) -> Result<Vec<u8>> {
        let sighash = tx.signing_hash();
        let secp = Secp256k1::signing_only();
        let msg = Message::from_digest_slice(&sighash)?;
        let sig = secp.sign_ecdsa_recoverable(&msg, &self.secret);
        let (rec_id, compact) = sig.serialize_compact();
        // EIP-1559 uses y_parity (0/1), NOT the legacy v = 27/28 + chain_id*2.
        // Emitting a legacy v here yields a transaction that recovers to the
        // wrong sender and is silently rejected by the builder.
        let y_parity = rec_id.to_i32() as u64;
        Ok(tx.encode_signed(&compact[..32], &compact[32..], y_parity))
    }
}

/// Keccak-256 of the uncompressed pubkey, last 20 bytes.
fn derive_address(secret: &SecretKey) -> [u8; 20] {
    let secp = Secp256k1::signing_only();
    let pubkey = secp256k1::PublicKey::from_secret_key(&secp, secret);
    let uncompressed = pubkey.serialize_uncompressed(); // 65 bytes, leading 0x04
    let hash = Keccak256::digest(&uncompressed[1..]);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    addr
}

/// An EIP-1559 (type 2) transaction.
#[derive(Debug, Clone)]
pub struct Eip1559Tx {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    /// `None` would be a contract creation; this path never does that.
    pub to: [u8; 20],
    pub value: u128,
    pub data: Vec<u8>,
}

impl Eip1559Tx {
    /// keccak256(0x02 || rlp([chain_id, nonce, maxPriority, maxFee, gas, to,
    /// value, data, accessList]))
    pub fn signing_hash(&self) -> [u8; 32] {
        let mut payload = vec![0x02u8];
        payload.extend_from_slice(&self.rlp_unsigned());
        let d = Keccak256::digest(&payload);
        let mut out = [0u8; 32];
        out.copy_from_slice(&d);
        out
    }

    fn rlp_unsigned(&self) -> Vec<u8> {
        let items = vec![
            rlp_uint(self.chain_id as u128),
            rlp_uint(self.nonce as u128),
            rlp_uint(self.max_priority_fee_per_gas),
            rlp_uint(self.max_fee_per_gas),
            rlp_uint(self.gas_limit as u128),
            rlp_bytes(&self.to),
            rlp_uint(self.value),
            rlp_bytes(&self.data),
            rlp_list(&[]), // empty access list
        ];
        rlp_list(&items.concat())
    }

    fn encode_signed(&self, r: &[u8], s: &[u8], y_parity: u64) -> Vec<u8> {
        let items = vec![
            rlp_uint(self.chain_id as u128),
            rlp_uint(self.nonce as u128),
            rlp_uint(self.max_priority_fee_per_gas),
            rlp_uint(self.max_fee_per_gas),
            rlp_uint(self.gas_limit as u128),
            rlp_bytes(&self.to),
            rlp_uint(self.value),
            rlp_bytes(&self.data),
            rlp_list(&[]),
            rlp_uint(y_parity as u128),
            rlp_bytes(trim_left(r)),
            rlp_bytes(trim_left(s)),
        ];
        let mut out = vec![0x02u8];
        out.extend_from_slice(&rlp_list(&items.concat()));
        out
    }
}

// ── Minimal RLP. Only what a type-2 transaction needs. ──────────────────────

fn trim_left(b: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < b.len() && b[i] == 0 {
        i += 1;
    }
    &b[i..]
}

fn rlp_uint(v: u128) -> Vec<u8> {
    if v == 0 {
        // RLP encodes zero as the EMPTY STRING (0x80), not as 0x00. Encoding it
        // as a zero byte changes the hash and produces a different sender.
        return vec![0x80];
    }
    let be = v.to_be_bytes();
    rlp_bytes(trim_left(&be))
}

fn rlp_bytes(b: &[u8]) -> Vec<u8> {
    if b.len() == 1 && b[0] < 0x80 {
        return b.to_vec();
    }
    let mut out = rlp_len(b.len(), 0x80);
    out.extend_from_slice(b);
    out
}

fn rlp_list(payload: &[u8]) -> Vec<u8> {
    let mut out = rlp_len(payload.len(), 0xc0);
    out.extend_from_slice(payload);
    out
}

fn rlp_len(len: usize, offset: u8) -> Vec<u8> {
    if len < 56 {
        vec![offset + len as u8]
    } else {
        let be = len.to_be_bytes();
        let t = trim_left(&be);
        let mut out = vec![offset + 55 + t.len() as u8];
        out.extend_from_slice(t);
        out
    }
}

/// The JSON-RPC body for submitting to the builder route.
///
/// Submit-only: the route re-sends signed bytes and never signs, so nothing
/// secret is in this payload.
pub fn send_raw_transaction_body(raw: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_sendRawTransaction",
        "params": [format!("0x{}", hex::encode(raw))],
    })
}

/// Build the builder URL for an api key.
///
/// Falls back to the shared default when no key is configured — and the CALLER
/// must tell the user, the same as every other endpoint.
pub fn builder_url(base: &str, api_key: Option<&str>) -> String {
    let key = api_key.unwrap_or("jsonrpc");
    format!("{}/v4/{}/ethereum-builder", base.trim_end_matches('/'), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Address derivation, against the canonical smallest-scalar key.
    ///
    /// If this is wrong every signature is attributed to the wrong sender, and
    /// the failure looks like "the builder rejected my transaction".
    #[test]
    fn address_derivation_matches_the_known_vector() {
        let k = EthKey::from_hex(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(k.address_hex(), "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf");
    }

    #[test]
    fn malformed_keys_are_refused_without_echoing_them() {
        for bad in ["", "0x", "deadbeef", "0xzz00000000000000000000000000000000000000000000000000000000000001"] {
            let e = EthKey::from_hex(bad).unwrap_err().to_string();
            assert!(!e.contains(bad) || bad.is_empty(), "error must not echo key material: {e}");
        }
        // The all-zero scalar is not a valid secp256k1 key.
        assert!(EthKey::from_hex(&format!("0x{}", "0".repeat(64))).is_err());
    }

    #[test]
    fn debug_prints_the_address_and_never_the_key() {
        let k = EthKey::from_hex(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let d = format!("{k:?}");
        assert!(d.contains("7e5f4552"));
        assert!(!d.contains("0000000000000000000000000000000000000000000000000000000000000001"));
    }

    /// RLP encodes zero as the empty string (0x80), not a zero byte.
    ///
    /// Getting this wrong changes the signing hash, so the transaction recovers
    /// to a different sender — and reads as an unexplained rejection.
    #[test]
    fn rlp_encodes_zero_as_empty_string() {
        assert_eq!(rlp_uint(0), vec![0x80]);
        assert_eq!(rlp_uint(1), vec![0x01]);
        assert_eq!(rlp_uint(0x7f), vec![0x7f]);
        assert_eq!(rlp_uint(0x80), vec![0x81, 0x80]);
        assert_eq!(rlp_uint(1024), vec![0x82, 0x04, 0x00]);
    }

    #[test]
    fn signing_produces_a_type_2_envelope_and_is_deterministic() {
        let k = EthKey::from_hex(
            "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318",
        )
        .unwrap();
        let tx = Eip1559Tx {
            chain_id: CHAIN_ID,
            nonce: 0,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 30_000_000_000,
            gas_limit: 120_000,
            to: hex::decode("95779e7e1c943042255b8a78273fe6de4823cf06")
                .unwrap()
                .try_into()
                .unwrap(),
            value: 0,
            data: hex::decode("bedb65ee").unwrap(),
            };
        let a = k.sign_1559(&tx).unwrap();
        let b = k.sign_1559(&tx).unwrap();
        assert_eq!(a, b, "RFC6979 signing must be deterministic");
        assert_eq!(a[0], 0x02, "must be a type-2 (EIP-1559) envelope");
        assert!(a.len() > 100);
    }

    /// The builder path is submit-only and must never be confused with the read
    /// path — `/ethereum` is a SUBSTRING of `/ethereum-builder`, and the edge
    /// config calls that ordering load-bearing for exactly this reason.
    #[test]
    fn builder_url_targets_the_private_submit_route() {
        let u = builder_url("https://mainnet.subfrost.io", Some("KEY"));
        assert_eq!(u, "https://mainnet.subfrost.io/v4/KEY/ethereum-builder");
        assert!(u.ends_with("/ethereum-builder"), "must not be the public read path");
        // No key: the shared default, which the caller has to announce.
        assert_eq!(
            builder_url("https://mainnet.subfrost.io/", None),
            "https://mainnet.subfrost.io/v4/jsonrpc/ethereum-builder"
        );
    }

    #[test]
    fn inline_key_is_refused_without_explicit_acknowledgement() {
        let e = EthKey::resolve(None, Some("0x01"), false).unwrap_err().to_string();
        assert!(e.contains("shell history"), "{e}");
    }

    #[test]
    fn missing_key_names_every_supported_mechanism() {
        // An agent that cannot find a key must be told how to supply one.
        std::env::remove_var("ETHEREUM_PRIVATE_KEY");
        let e = EthKey::resolve(None, None, false).unwrap_err().to_string();
        assert!(e.contains("--eth-key-file"));
        assert!(e.contains("ETHEREUM_PRIVATE_KEY"));
        assert!(e.contains("--eth-private-key"));
    }
}
