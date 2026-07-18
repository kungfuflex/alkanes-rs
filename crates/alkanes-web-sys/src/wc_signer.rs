//! Wasm-bindgen exports of the new-protocol WalletConnect signer's
//! codec layer.
//!
//! What's exposed here:
//!   * `generateDappKeypair` — mint a fresh X25519 keypair (priv+pub b64url)
//!   * `generatePairingCode` — 6-char human-typeable pairing code
//!   * `generateCliPeerName` — `frtun1<32hex>.peer`
//!   * `buildPairingUri`    — `subfrost://wc/<peer>?key=...&code=...`
//!   * `parsePairingUri`    — same, in reverse
//!   * `deriveSymKey`       — X25519 ECDH + HKDF-SHA256 to 32-byte symKey
//!   * `buildSignPsbtRequest` / `buildSignMessageRequest` /
//!     `buildGetAccountsRequest` — produce `{ciphertextB64, nonceB64}`
//!   * `decryptEnvelope`    — round-trip a response envelope
//!   * `WC_DEFAULT_BRIDGE_URL` — `wss://wss-tls.subfrost.io/v1/pair`
//!
//! Intentional scope: codec only. Transport (browser WebSocket dial +
//! `fetch` for /v1/pair-wake) + storage (IndexedDB) live in JS land on
//! the `@alkanes/ts-sdk` side. Mirrors how
//! `~/subfrost-mobile/ts-sdk/src/cli.ts` is built: TS owns the I/O,
//! Rust+wasm owns the bytes. Future iteration can lift WasmTransport +
//! IndexedDbStorage into Rust if we want a single ship-binary.

use alkanes_cli_common::wc_signer::{
    crypto::{self, KEY_LEN},
    pairing,
    wire::{Plaintext, WireEnvelope},
};
use base64::Engine;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------

#[wasm_bindgen(js_name = wcDefaultBridgeUrl)]
pub fn wc_default_bridge_url() -> String {
    "wss://wss-tls.subfrost.io/v1/pair".to_string()
}

// ---------------------------------------------------------------------
// Identity mints
// ---------------------------------------------------------------------

/// Mint a fresh X25519 keypair, return `{pub_b64, priv_b64}` as JS.
#[wasm_bindgen(js_name = wcGenerateDappKeypair)]
pub fn wc_generate_dapp_keypair() -> JsValue {
    let (priv_key, pub_key) = crypto::gen_keypair();
    let pub_b64 = crypto::pub_to_b64url(&pub_key);
    let priv_b64 = crypto::b64url_encode(priv_key.as_bytes());
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("pub_b64"), &JsValue::from_str(&pub_b64));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("priv_b64"), &JsValue::from_str(&priv_b64));
    obj.into()
}

#[wasm_bindgen(js_name = wcGeneratePairingCode)]
pub fn wc_generate_pairing_code() -> String {
    pairing::generate_pairing_code()
}

#[wasm_bindgen(js_name = wcGenerateCliPeerName)]
pub fn wc_generate_cli_peer_name() -> String {
    pairing::generate_cli_peer_name()
}

// ---------------------------------------------------------------------
// URI build / parse
// ---------------------------------------------------------------------

#[wasm_bindgen(js_name = wcBuildPairingUri)]
pub fn wc_build_pairing_uri(
    cli_peer: &str,
    dapp_pub_b64: &str,
    pairing_code: &str,
    bridge_url: &str,
    origin: &str,
    mode: &str,
) -> String {
    pairing::build_pair_uri(cli_peer, dapp_pub_b64, pairing_code, bridge_url, origin, mode)
}

#[wasm_bindgen(js_name = wcParsePairingUri)]
pub fn wc_parse_pairing_uri(uri: &str) -> Result<JsValue, JsValue> {
    let p = pairing::parse_pair_uri(uri)
        .map_err(|e| JsValue::from_str(&format!("parse_pairing_uri: {e}")))?;
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("cli_peer"), &JsValue::from_str(&p.cli_peer));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("dapp_pub_b64"), &JsValue::from_str(&p.dapp_pub_b64));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("pairing_code"), &JsValue::from_str(&p.pairing_code));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("bridge_url"), &JsValue::from_str(&p.bridge_url));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("origin"), &JsValue::from_str(&p.origin));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("mode"), &JsValue::from_str(&p.mode));
    Ok(obj.into())
}

// ---------------------------------------------------------------------
// ECDH / symKey
// ---------------------------------------------------------------------

/// Derive the shared 32-byte symKey from (own_priv_b64, peer_pub_b64,
/// info). Returns base64url(32B). `info` for the new protocol is
/// `<phone_peer>:<pairing_code>`.
#[wasm_bindgen(js_name = wcDeriveSymKey)]
pub fn wc_derive_sym_key(own_priv_b64: &str, peer_pub_b64: &str, info: &str) -> Result<String, JsValue> {
    let priv_bytes = crypto::b64url_decode(own_priv_b64)
        .map_err(|e| JsValue::from_str(&format!("priv b64: {e}")))?;
    if priv_bytes.len() != 32 {
        return Err(JsValue::from_str(&format!("priv length {} != 32", priv_bytes.len())));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&priv_bytes);
    let priv_key = x25519_dalek::StaticSecret::from(arr);
    let peer_pub = crypto::pub_from_b64url(peer_pub_b64)
        .map_err(|e| JsValue::from_str(&format!("peer pub: {e}")))?;
    let sym = crypto::ecdh_derive(&priv_key, &peer_pub, info)
        .map_err(|e| JsValue::from_str(&format!("ecdh: {e}")))?;
    Ok(crypto::b64url_encode(&sym))
}

// ---------------------------------------------------------------------
// Request builders — produce the camelCase `{ciphertextB64, nonceB64}`
// shape the bridge ferries.
// ---------------------------------------------------------------------

fn sym_key_from_b64(sym_b64: &str) -> Result<[u8; KEY_LEN], JsValue> {
    let bytes = crypto::b64url_decode(sym_b64)
        .map_err(|e| JsValue::from_str(&format!("sym b64: {e}")))?;
    if bytes.len() != KEY_LEN {
        return Err(JsValue::from_str(&format!("sym len {} != {}", bytes.len(), KEY_LEN)));
    }
    let mut arr = [0u8; KEY_LEN];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn envelope_to_js(env: &WireEnvelope) -> JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("ciphertextB64"), &JsValue::from_str(&env.ciphertext_b64));
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("nonceB64"), &JsValue::from_str(&env.nonce_b64));
    obj.into()
}

#[wasm_bindgen(js_name = wcBuildSignPsbtRequest)]
pub fn wc_build_sign_psbt_request(
    sym_key_b64: &str,
    psbt_hex: &str,
    addresses_json: &str,
    origin: &str,
    request_id: &str,
) -> Result<JsValue, JsValue> {
    let sym = sym_key_from_b64(sym_key_b64)?;
    let addresses: Vec<String> = serde_json::from_str(addresses_json)
        .map_err(|e| JsValue::from_str(&format!("addresses json: {e}")))?;
    let req = Plaintext::SignPsbt {
        psbt_hex: psbt_hex.to_string(),
        addresses,
        request_id: request_id.to_string(),
        origin: origin.to_string(),
    };
    let req_json = serde_json::to_vec(&req)
        .map_err(|e| JsValue::from_str(&format!("serialize req: {e}")))?;
    let env = crypto::encrypt_to_envelope(&sym, &req_json)
        .map_err(|e| JsValue::from_str(&format!("encrypt: {e}")))?;
    Ok(envelope_to_js(&env))
}

#[wasm_bindgen(js_name = wcBuildSignMessageRequest)]
pub fn wc_build_sign_message_request(
    sym_key_b64: &str,
    message: &str,
    address: &str,
    origin: &str,
    request_id: &str,
) -> Result<JsValue, JsValue> {
    let sym = sym_key_from_b64(sym_key_b64)?;
    let req = Plaintext::SignMessage {
        message: message.to_string(),
        address: address.to_string(),
        request_id: request_id.to_string(),
        origin: origin.to_string(),
    };
    let req_json = serde_json::to_vec(&req)
        .map_err(|e| JsValue::from_str(&format!("serialize req: {e}")))?;
    let env = crypto::encrypt_to_envelope(&sym, &req_json)
        .map_err(|e| JsValue::from_str(&format!("encrypt: {e}")))?;
    Ok(envelope_to_js(&env))
}

#[wasm_bindgen(js_name = wcBuildGetAccountsRequest)]
pub fn wc_build_get_accounts_request(
    sym_key_b64: &str,
    origin: &str,
    request_id: &str,
) -> Result<JsValue, JsValue> {
    let sym = sym_key_from_b64(sym_key_b64)?;
    let req = Plaintext::GetAccounts {
        request_id: request_id.to_string(),
        origin: origin.to_string(),
    };
    let req_json = serde_json::to_vec(&req)
        .map_err(|e| JsValue::from_str(&format!("serialize req: {e}")))?;
    let env = crypto::encrypt_to_envelope(&sym, &req_json)
        .map_err(|e| JsValue::from_str(&format!("encrypt: {e}")))?;
    Ok(envelope_to_js(&env))
}

// ---------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------

/// Decrypt + parse a response envelope. Returns the Plaintext JSON
/// string for the JS side to `JSON.parse`.
#[wasm_bindgen(js_name = wcDecryptEnvelope)]
pub fn wc_decrypt_envelope(
    sym_key_b64: &str,
    ciphertext_b64: &str,
    nonce_b64: &str,
) -> Result<String, JsValue> {
    let sym = sym_key_from_b64(sym_key_b64)?;
    let env = WireEnvelope {
        ciphertext_b64: ciphertext_b64.to_string(),
        nonce_b64: nonce_b64.to_string(),
    };
    let pt = crypto::decrypt_envelope(&sym, &env)
        .map_err(|e| JsValue::from_str(&format!("decrypt: {e}")))?;
    // Round-trip through Plaintext to validate shape, then ship the
    // canonical tagged-union JSON the TS side expects.
    let plain: Plaintext = serde_json::from_slice(&pt)
        .map_err(|e| JsValue::from_str(&format!("parse plaintext: {e}")))?;
    serde_json::to_string(&plain)
        .map_err(|e| JsValue::from_str(&format!("re-serialize: {e}")))
}

// Silence the unused-import lint when this module is built but no
// consumer pulls in a function.
#[allow(dead_code)]
fn _silence() {
    let _ = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 0]);
}
