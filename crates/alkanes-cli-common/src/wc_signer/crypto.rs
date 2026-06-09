//! Symmetric crypto for the new WC protocol.
//!
//! Per-pairing flow (matches `subfrost-wallet-wc::crypto` byte-for-byte,
//! and the TS-side @noble/ciphers + @noble/hashes + @noble/curves stack):
//!   1. dapp + phone each generate an ephemeral X25519 keypair.
//!   2. Pubkeys exchanged over the bridge (dapp's via the deeplink
//!      `key=` param; phone's via the first binary frame after `listen`
//!      accepts the dial).
//!   3. ECDH(my_priv, their_pub) → 32-byte shared secret.
//!   4. HKDF-SHA256(shared, salt = "subfrost-wc-v1",
//!                            info = "<phone_peer>:<pairing_code>")
//!      → 32-byte symmetric key.
//!   5. ChaCha20-Poly1305(symKey, nonce=12B random, plaintext).
//!
//! The HKDF info string is the load-bearing binding — a passive
//! eavesdropper on the bridge can't infer symKey without the 6-char
//! pairing code the user typed into the phone.

use base64::Engine;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
const HKDF_SALT: &[u8] = b"subfrost-wc-v1";

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid public key length: expected 32, got {0}")]
    BadPubLen(usize),
    #[error("invalid nonce length: expected 12, got {0}")]
    BadNonceLen(usize),
    #[error("base64url decode: {0}")]
    BadBase64(String),
    #[error("aead encrypt failed")]
    EncryptFail,
    #[error("aead decrypt failed (auth tag mismatch?)")]
    DecryptFail,
    #[error("hkdf expand")]
    HkdfFail,
}

/// Generate a fresh X25519 keypair. The secret stays in memory only.
pub fn gen_keypair() -> (StaticSecret, PublicKey) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let s = StaticSecret::from(bytes);
    let p = PublicKey::from(&s);
    (s, p)
}

/// Derive the shared symmetric key. `info` is the HKDF info string —
/// for the new protocol this is `"{phone_peer}:{pairing_code}"`.
pub fn ecdh_derive(
    my_priv: &StaticSecret,
    their_pub: &PublicKey,
    info: &str,
) -> Result<[u8; KEY_LEN], CryptoError> {
    let shared = my_priv.diffie_hellman(their_pub);
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared.as_bytes());
    let mut out = [0u8; KEY_LEN];
    hk.expand(info.as_bytes(), &mut out)
        .map_err(|_| CryptoError::HkdfFail)?;
    Ok(out)
}

/// Encrypt with a fresh random nonce. Returns `(ciphertext, nonce)`.
pub fn encrypt(
    sym_key: &[u8; KEY_LEN],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; NONCE_LEN]), CryptoError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(sym_key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptFail)?;
    Ok((ct, nonce_bytes))
}

/// Decrypt a ciphertext with the supplied 12-byte nonce.
pub fn decrypt(
    sym_key: &[u8; KEY_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if nonce.len() != NONCE_LEN {
        return Err(CryptoError::BadNonceLen(nonce.len()));
    }
    let cipher = ChaCha20Poly1305::new(Key::from_slice(sym_key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| CryptoError::DecryptFail)
}

pub fn pub_from_b64url(b64: &str) -> Result<PublicKey, CryptoError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64.trim())
        .map_err(|e| CryptoError::BadBase64(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(CryptoError::BadPubLen(bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(PublicKey::from(arr))
}

pub fn pub_to_b64url(p: &PublicKey) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(p.as_bytes())
}

pub fn b64url_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>, CryptoError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s.trim())
        .map_err(|e| CryptoError::BadBase64(e.to_string()))
}

/// Convenience: encrypt a plaintext and produce the camelCase envelope
/// the wire shipping code expects.
pub fn encrypt_to_envelope(
    sym_key: &[u8; KEY_LEN],
    plaintext: &[u8],
) -> Result<crate::wc_signer::wire::WireEnvelope, CryptoError> {
    let (ct, nonce) = encrypt(sym_key, plaintext)?;
    Ok(crate::wc_signer::wire::WireEnvelope {
        ciphertext_b64: b64url_encode(&ct),
        nonce_b64: b64url_encode(&nonce),
    })
}

/// Convenience: decrypt a wire envelope back to plaintext bytes.
pub fn decrypt_envelope(
    sym_key: &[u8; KEY_LEN],
    env: &crate::wc_signer::wire::WireEnvelope,
) -> Result<Vec<u8>, CryptoError> {
    let ct = b64url_decode(&env.ciphertext_b64)?;
    let nonce = b64url_decode(&env.nonce_b64)?;
    decrypt(sym_key, &nonce, &ct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypto_roundtrip() {
        let (a_priv, a_pub) = gen_keypair();
        let (b_priv, b_pub) = gen_keypair();
        let info = "phone-peer:ABCDEF";
        let key_a = ecdh_derive(&a_priv, &b_pub, info).unwrap();
        let key_b = ecdh_derive(&b_priv, &a_pub, info).unwrap();
        assert_eq!(key_a, key_b, "ecdh must converge");

        let pt = b"hello, signer";
        let (ct, nonce) = encrypt(&key_a, pt).unwrap();
        let dec = decrypt(&key_b, &nonce, &ct).unwrap();
        assert_eq!(dec, pt);
    }

    #[test]
    fn envelope_roundtrip_via_helpers() {
        let (a_priv, a_pub) = gen_keypair();
        let (b_priv, b_pub) = gen_keypair();
        let info = "phone-peer:ABCDEF";
        let key_a = ecdh_derive(&a_priv, &b_pub, info).unwrap();
        let key_b = ecdh_derive(&b_priv, &a_pub, info).unwrap();
        let pt = b"hello";
        let env = encrypt_to_envelope(&key_a, pt).unwrap();
        let back = decrypt_envelope(&key_b, &env).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn hkdf_info_binds_to_pairing_code() {
        let (a_priv, _a_pub) = gen_keypair();
        let (_b_priv, b_pub) = gen_keypair();
        let k1 = ecdh_derive(&a_priv, &b_pub, "phone-1:ABCDEF").unwrap();
        let k2 = ecdh_derive(&a_priv, &b_pub, "phone-1:WRONG6").unwrap();
        assert_ne!(k1, k2, "different pairing code must derive different symKey");
    }

    #[test]
    fn b64url_round_trip_no_padding() {
        let raw = [0x01, 0x02, 0x03];
        let enc = b64url_encode(&raw);
        assert!(!enc.contains('='));
        let dec = b64url_decode(&enc).unwrap();
        assert_eq!(dec, raw);
    }

    #[test]
    fn pub_b64_round_trip() {
        let (_priv, pubk) = gen_keypair();
        let s = pub_to_b64url(&pubk);
        let back = pub_from_b64url(&s).unwrap();
        assert_eq!(back.as_bytes(), pubk.as_bytes());
    }

    #[test]
    fn bad_nonce_length_is_rejected() {
        let key = [0u8; KEY_LEN];
        let r = decrypt(&key, &[0u8; 8], b"x");
        assert!(matches!(r, Err(CryptoError::BadNonceLen(8))));
    }
}
