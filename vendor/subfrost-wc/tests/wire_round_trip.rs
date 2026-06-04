//! End-to-end wire-shape contract test for the canonical alkanes-rs
//! copy of subfrost-wc.
//!
//! The subfrost-mobile crate has its own headless e2e in
//! `subfrost-wallet-integ-tests/tests/wc_headless_e2e.rs` that drives
//! the *mobile FFI* layer end-to-end. This test sits at the layer
//! below — it exercises only `subfrost-wc` (the wire types + crypto)
//! and pins the contract that BOTH peers must speak:
//!
//!   1. ECDH pairing produces a shared symKey both sides agree on.
//!   2. The Plaintext enum's snake_case JSON layout is stable
//!      (any rename breaks every shipped client — alkanes-cli, the
//!      webapp's `lib/wc/client.ts`, the bindgen-cli extension copy,
//!      the mobile FFI).
//!   3. encrypt(symKey, plaintext) → decrypt(symKey, ct, nonce)
//!      round-trips byte-for-byte across the full request/response
//!      cycle.
//!
//! If a future refactor of the wire enum changes a field name or
//! tag value, this test is what fires. The wire shape is contractual
//! across four shipped clients; protecting it is the whole point.
//!
//! Out of scope (covered by the live subfrost-wallet-integ-tests
//! suite or by manual staging probes):
//!   * the wc-relay HTTP routes,
//!   * the FCM-wake `/v1/pair-wake` path,
//!   * BIP-137 / BIP-340 signature verification,
//!   * the alkanes-cli `WcRemoteSigner` adapter trait shape.

use serde_json::{json, Value};
use subfrost_wc::{
    crypto::{decrypt, ecdh_derive, encrypt, gen_keypair},
    wire::Plaintext,
};

/// Webapp / CLI / mobile all speak this exact JSON shape. Tag is
/// `type` (lowercased snake_case); the typed fields under each variant
/// are flattened siblings of `type`.
#[test]
fn plaintext_sign_psbt_request_json_shape() {
    let pt = Plaintext::SignPsbt {
        psbt_hex:   "70736274ff01".into(),
        addresses:  vec!["bc1qexample".into()],
        request_id: "00000000-0000-0000-0000-000000000001".into(),
        origin:     "https://app.subfrost.io".into(),
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "sign_psbt");
    assert_eq!(v["psbt_hex"],   "70736274ff01");
    assert_eq!(v["addresses"],  json!(["bc1qexample"]));
    assert_eq!(v["request_id"], "00000000-0000-0000-0000-000000000001");
    assert_eq!(v["origin"],     "https://app.subfrost.io");
}

#[test]
fn plaintext_sign_message_request_json_shape() {
    let pt = Plaintext::SignMessage {
        message:    "hello".into(),
        address:    "bc1pexample".into(),
        request_id: "rid-1".into(),
        origin:     "https://app.subfrost.io".into(),
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "sign_message");
    assert_eq!(v["message"],    "hello");
    assert_eq!(v["address"],    "bc1pexample");
    assert_eq!(v["request_id"], "rid-1");
}

#[test]
fn plaintext_get_accounts_request_json_shape() {
    let pt = Plaintext::GetAccounts {
        request_id: "rid-2".into(),
        origin:     "https://app.subfrost.io".into(),
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "get_accounts");
    assert_eq!(v["request_id"], "rid-2");
    assert_eq!(v["origin"],     "https://app.subfrost.io");
}

#[test]
fn plaintext_result_response_json_shape() {
    let pt = Plaintext::Result {
        request_id: "rid-1".into(),
        result:     "70736274ff01signed".into(),
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "result");
    assert_eq!(v["request_id"], "rid-1");
    assert_eq!(v["result"],     "70736274ff01signed");
}

#[test]
fn plaintext_error_response_json_shape() {
    let pt = Plaintext::Error {
        request_id: "rid-1".into(),
        code:       "user_rejected".into(),
        message:    "user rejected the request".into(),
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "error");
    assert_eq!(v["code"],       "user_rejected");
    assert_eq!(v["message"],    "user rejected the request");
}

#[test]
fn plaintext_accounts_response_json_shape() {
    let pt = Plaintext::Accounts {
        request_id: "rid-2".into(),
        addresses:  vec!["bc1qa".into(), "bc1pa".into()],
    };
    let v: Value = serde_json::to_value(&pt).unwrap();
    assert_eq!(v["type"],       "accounts");
    assert_eq!(v["addresses"],  json!(["bc1qa", "bc1pa"]));
}

/// Mirror the full webapp→mobile→webapp signPsbt cycle at the wire
/// layer:
///
///   * webapp generates kp_web
///   * mobile generates kp_mob (driven by `parse_pairing_uri` in
///     production; here we shortcut to gen_keypair since `wire_round_trip`
///     doesn't exercise URI parsing — `pairing.rs` has its own tests)
///   * both sides ECDH-derive the same symKey using the same topic
///   * webapp encrypts a SignPsbt request, mobile decrypts + matches
///   * mobile encrypts a Result response, webapp decrypts + matches
///
/// Asserts every layer round-trips byte-identical. This is the
/// contract that protects all four shipped clients (mobile FFI,
/// alkanes-cli, webapp `lib/wc/client.ts`, extension bindgen-cli).
#[test]
fn full_round_trip_sign_psbt() {
    let topic = "topic-abc-123";

    // 1. Both sides generate keypairs.
    let (web_priv, web_pub) = gen_keypair();
    let (mob_priv, mob_pub) = gen_keypair();

    // 2. ECDH — symKey on each side derived independently must match.
    let web_key = ecdh_derive(&web_priv, &mob_pub, topic).expect("web ecdh");
    let mob_key = ecdh_derive(&mob_priv, &web_pub, topic).expect("mob ecdh");
    assert_eq!(web_key, mob_key, "ECDH must converge on both sides");

    // 3. Webapp builds a SignPsbt request and encrypts.
    let req = Plaintext::SignPsbt {
        psbt_hex:   "70736274ff01000000".into(),
        addresses:  vec!["bc1qexample".into()],
        request_id: "rid-roundtrip".into(),
        origin:     "https://app.subfrost.io".into(),
    };
    let req_bytes = serde_json::to_vec(&req).unwrap();
    let (req_ct, req_nonce) = encrypt(&web_key, &req_bytes).expect("web encrypt");

    // 4. Mobile decrypts, parses, matches.
    let recovered = decrypt(&mob_key, &req_nonce, &req_ct).expect("mob decrypt");
    assert_eq!(recovered, req_bytes, "ciphertext round-trips byte-identical");
    let parsed: Plaintext = serde_json::from_slice(&recovered).expect("mob parse");
    match parsed {
        Plaintext::SignPsbt { psbt_hex, request_id, origin, addresses } => {
            assert_eq!(psbt_hex,    "70736274ff01000000");
            assert_eq!(request_id,  "rid-roundtrip");
            assert_eq!(origin,      "https://app.subfrost.io");
            assert_eq!(addresses,   vec!["bc1qexample".to_string()]);
        }
        other => panic!("expected SignPsbt, got {other:?}"),
    }

    // 5. Mobile builds a Result response, encrypts.
    let resp = Plaintext::Result {
        request_id: "rid-roundtrip".into(),
        result:     "70736274ff01signed".into(),
    };
    let resp_bytes = serde_json::to_vec(&resp).unwrap();
    let (resp_ct, resp_nonce) = encrypt(&mob_key, &resp_bytes).expect("mob encrypt");

    // 6. Webapp decrypts, parses, matches.
    let back = decrypt(&web_key, &resp_nonce, &resp_ct).expect("web decrypt");
    assert_eq!(back, resp_bytes);
    let parsed_resp: Plaintext = serde_json::from_slice(&back).expect("web parse");
    match parsed_resp {
        Plaintext::Result { request_id, result } => {
            assert_eq!(request_id, "rid-roundtrip");
            assert_eq!(result,     "70736274ff01signed");
        }
        other => panic!("expected Result, got {other:?}"),
    }
}

/// Negative: a tampered ciphertext (one byte flip) must fail decrypt —
/// ChaCha20-Poly1305 MUST reject the AEAD. This is what makes
/// "untrusted relay sees nothing" actually true; if Poly1305 ever
/// gets bypassed, the relay turns into a downgrade-attack target.
#[test]
fn tampered_ciphertext_fails_decrypt() {
    let topic = "topic-xyz";
    let (a_priv, a_pub) = gen_keypair();
    let (b_priv, b_pub) = gen_keypair();
    let a_key = ecdh_derive(&a_priv, &b_pub, topic).unwrap();
    let b_key = ecdh_derive(&b_priv, &a_pub, topic).unwrap();
    assert_eq!(a_key, b_key);

    let pt = b"sensitive payload";
    let (mut ct, nonce) = encrypt(&a_key, pt).unwrap();

    // Flip last byte (Poly1305 tag region).
    let last = ct.len() - 1;
    ct[last] ^= 0x01;

    let result = decrypt(&b_key, &nonce, &ct);
    assert!(result.is_err(), "tampered ciphertext must fail decrypt");
}

/// Negative: wrong sym key (a relay impersonating one side with a
/// known-bad symKey) cannot decrypt.
#[test]
fn wrong_key_fails_decrypt() {
    let topic = "topic-xyz";
    let (a_priv, _) = gen_keypair();
    let (_, b_pub)  = gen_keypair();
    let (_, c_pub)  = gen_keypair();
    let real_key  = ecdh_derive(&a_priv, &b_pub, topic).unwrap();
    let wrong_key = ecdh_derive(&a_priv, &c_pub, topic).unwrap();
    assert_ne!(real_key, wrong_key);

    let pt = b"hello world";
    let (ct, nonce) = encrypt(&real_key, pt).unwrap();
    let result = decrypt(&wrong_key, &nonce, &ct);
    assert!(result.is_err(), "wrong key must fail decrypt");
}

/// Negative: a topic change between the two ECDH derivations changes
/// the derived symKey. This is what ties a symKey to a specific
/// pairing — the relay sees the topic in cleartext but cannot reuse
/// a captured symKey across pairings.
#[test]
fn topic_mismatch_diverges_keys() {
    let (a_priv, a_pub) = gen_keypair();
    let (b_priv, b_pub) = gen_keypair();
    let k_topic1 = ecdh_derive(&a_priv, &b_pub, "topic-1").unwrap();
    let k_topic2 = ecdh_derive(&b_priv, &a_pub, "topic-2").unwrap();
    assert_ne!(k_topic1, k_topic2, "topic mixes into HKDF; mismatched topics must diverge");
}
