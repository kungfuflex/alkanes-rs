//! Cryptographic key types for Qubitcoin.
//!
//! Maps to: `src/key.h` and `src/pubkey.h` in Bitcoin Core.
//!
//! Provides:
//! - `Key`: A private key wrapping `secp256k1::SecretKey`.
//! - `PubKey`: A public key wrapping `secp256k1::PublicKey`.
//! - `XOnlyPubKey`: An x-only public key for Taproot/BIP340.

use qubitcoin_crypto::hash::hash160;
use qubitcoin_primitives::Uint256;

/// A private key (wraps `secp256k1::SecretKey`).
///
/// Port of Bitcoin Core's `CKey` from `src/key.h`.
pub struct Key {
    inner: secp256k1::SecretKey,
    compressed: bool,
}

impl Clone for Key {
    fn clone(&self) -> Self {
        let bytes = self.inner.secret_bytes();
        Key {
            inner: secp256k1::SecretKey::from_slice(&bytes)
                .expect("cloning a valid secret key should never fail"),
            compressed: self.compressed,
        }
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key")
            .field("compressed", &self.compressed)
            .finish_non_exhaustive()
    }
}

impl Key {
    /// Create a new private key from raw bytes.
    ///
    /// `data` must be exactly 32 bytes representing a valid secp256k1 scalar.
    /// `compressed` controls whether the derived public key will be serialized
    /// in compressed (33-byte) or uncompressed (65-byte) form.
    pub fn new(data: &[u8], compressed: bool) -> Result<Self, secp256k1::Error> {
        let inner = secp256k1::SecretKey::from_slice(data)?;
        Ok(Key { inner, compressed })
    }

    /// Generate a new random private key (compressed by default).
    #[cfg(feature = "native")]
    pub fn generate() -> Self {
        let secp = secp256k1::Secp256k1::new();
        let (secret_key, _) = secp.generate_keypair(&mut rand::thread_rng());
        Key {
            inner: secret_key,
            compressed: true,
        }
    }

    /// Derive the public key corresponding to this private key.
    pub fn get_pubkey(&self) -> PubKey {
        let secp = secp256k1::Secp256k1::new();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &self.inner);
        PubKey {
            inner: pk,
            compressed: self.compressed,
        }
    }

    /// Create an ECDSA signature over a 256-bit hash.
    ///
    /// Returns the DER-encoded signature bytes.
    pub fn sign(&self, hash: &Uint256) -> Result<Vec<u8>, secp256k1::Error> {
        let secp = secp256k1::Secp256k1::new();
        let msg = secp256k1::Message::from_digest_slice(hash.as_bytes())?;
        let sig = secp.sign_ecdsa(&msg, &self.inner);
        Ok(sig.serialize_der().to_vec())
    }

    /// Create a Schnorr (BIP340) signature over a 256-bit hash.
    ///
    /// Returns the 64-byte raw signature.
    pub fn sign_schnorr(&self, hash: &Uint256) -> Result<[u8; 64], secp256k1::Error> {
        let secp = secp256k1::Secp256k1::new();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &self.inner);
        let msg = secp256k1::Message::from_digest_slice(hash.as_bytes())?;
        #[cfg(feature = "rand")]
        let sig = secp.sign_schnorr(&msg, &keypair);
        #[cfg(not(feature = "rand"))]
        let sig = secp.sign_schnorr_no_aux_rand(&msg, &keypair);
        Ok(sig.serialize())
    }

    /// Get a reference to the inner `secp256k1::SecretKey`.
    pub fn inner(&self) -> &secp256k1::SecretKey {
        &self.inner
    }

    /// Whether this key produces compressed public keys.
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// Return the 32-byte secret key material.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.inner.secret_bytes()
    }
}

/// A public key (wraps `secp256k1::PublicKey`).
///
/// Port of Bitcoin Core's `CPubKey` from `src/pubkey.h`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubKey {
    inner: secp256k1::PublicKey,
    compressed: bool,
}

impl PubKey {
    /// Parse a public key from a serialized byte slice.
    ///
    /// Accepts both compressed (33-byte) and uncompressed (65-byte) encodings.
    pub fn from_slice(data: &[u8]) -> Result<Self, secp256k1::Error> {
        let inner = secp256k1::PublicKey::from_slice(data)?;
        let compressed = data.len() == 33;
        Ok(PubKey { inner, compressed })
    }

    /// Serialize the public key to bytes.
    ///
    /// Returns 33 bytes if compressed, 65 bytes if uncompressed.
    pub fn serialize(&self) -> Vec<u8> {
        if self.compressed {
            self.inner.serialize().to_vec()
        } else {
            self.inner.serialize_uncompressed().to_vec()
        }
    }

    /// Compute the key ID: `HASH160(serialized_pubkey)`.
    ///
    /// This is the 20-byte identifier used in P2PKH addresses.
    pub fn get_id(&self) -> [u8; 20] {
        hash160(&self.serialize())
    }

    /// Verify an ECDSA signature over a 256-bit hash.
    ///
    /// `sig` must be DER-encoded.
    pub fn verify(&self, hash: &Uint256, sig: &[u8]) -> bool {
        let secp = secp256k1::Secp256k1::new();
        let msg = match secp256k1::Message::from_digest_slice(hash.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let signature = match secp256k1::ecdsa::Signature::from_der(sig) {
            Ok(s) => s,
            Err(_) => return false,
        };
        secp.verify_ecdsa(&msg, &signature, &self.inner).is_ok()
    }

    /// Whether this public key is in compressed form.
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// Whether this public key is valid.
    ///
    /// Always returns `true` because `secp256k1::PublicKey` validates on construction.
    pub fn is_valid(&self) -> bool {
        true
    }

    /// Get a reference to the inner `secp256k1::PublicKey`.
    pub fn inner(&self) -> &secp256k1::PublicKey {
        &self.inner
    }
}

/// X-only public key (for Taproot, BIP340).
///
/// Port of Bitcoin Core's `XOnlyPubKey` from `src/pubkey.h`.
/// An x-only key is the 32-byte x-coordinate of a point on the curve,
/// used in Schnorr signatures and Taproot key paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XOnlyPubKey {
    inner: secp256k1::XOnlyPublicKey,
}

impl XOnlyPubKey {
    /// Parse an x-only public key from a 32-byte slice.
    pub fn from_slice(data: &[u8]) -> Result<Self, secp256k1::Error> {
        let inner = secp256k1::XOnlyPublicKey::from_slice(data)?;
        Ok(XOnlyPubKey { inner })
    }

    /// Create an x-only public key from a full public key.
    ///
    /// Drops the parity information, keeping only the x-coordinate.
    pub fn from_pubkey(pubkey: &PubKey) -> Self {
        let (xonly, _parity) = pubkey.inner().x_only_public_key();
        XOnlyPubKey { inner: xonly }
    }

    /// Serialize to a 32-byte array (the x-coordinate).
    pub fn serialize(&self) -> [u8; 32] {
        self.inner.serialize()
    }

    /// Verify a Schnorr (BIP340) signature over a 256-bit hash.
    pub fn verify_schnorr(&self, hash: &Uint256, sig: &[u8; 64]) -> bool {
        let secp = secp256k1::Secp256k1::new();
        let msg = match secp256k1::Message::from_digest_slice(hash.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let signature = match secp256k1::schnorr::Signature::from_slice(sig) {
            Ok(s) => s,
            Err(_) => return false,
        };
        secp.verify_schnorr(&signature, &msg, &self.inner).is_ok()
    }

    /// Get a reference to the inner `secp256k1::XOnlyPublicKey`.
    pub fn inner(&self) -> &secp256k1::XOnlyPublicKey {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_crypto::hash::hash160;

    #[test]
    fn test_generate_key_and_get_pubkey_roundtrip() {
        let key = Key::generate();
        assert!(key.is_compressed());

        let pubkey = key.get_pubkey();
        assert!(pubkey.is_valid());
        assert!(pubkey.is_compressed());

        // Serialized compressed pubkey should be 33 bytes, starting with 0x02 or 0x03.
        let serialized = pubkey.serialize();
        assert_eq!(serialized.len(), 33);
        assert!(serialized[0] == 0x02 || serialized[0] == 0x03);

        // Round-trip through PubKey::from_slice should yield the same key.
        let pubkey2 = PubKey::from_slice(&serialized).unwrap();
        assert_eq!(pubkey, pubkey2);
    }

    #[test]
    fn test_sign_and_verify_ecdsa() {
        let key = Key::generate();
        let pubkey = key.get_pubkey();

        // Create a hash to sign (just 32 bytes of test data).
        let hash = Uint256::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ]);

        let sig = key.sign(&hash).expect("signing should succeed");
        assert!(pubkey.verify(&hash, &sig), "signature should verify");

        // Tamper with the hash and verify it fails.
        let bad_hash = Uint256::from_bytes([0xff; 32]);
        assert!(
            !pubkey.verify(&bad_hash, &sig),
            "tampered hash should fail verification"
        );
    }

    #[test]
    fn test_sign_and_verify_schnorr() {
        let key = Key::generate();
        let pubkey = key.get_pubkey();
        let xonly = XOnlyPubKey::from_pubkey(&pubkey);

        let hash = Uint256::from_bytes([
            0xaa, 0xbb, 0xcc, 0xdd, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a,
            0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x1b, 0x1c,
        ]);

        let sig = key
            .sign_schnorr(&hash)
            .expect("schnorr signing should succeed");
        assert_eq!(sig.len(), 64);
        assert!(
            xonly.verify_schnorr(&hash, &sig),
            "schnorr signature should verify"
        );

        // Tamper with the hash and verify it fails.
        let bad_hash = Uint256::from_bytes([0x00; 32]);
        // Zero hash is invalid for Message::from_digest_slice, so verify returns false.
        assert!(
            !xonly.verify_schnorr(&bad_hash, &sig),
            "tampered hash should fail schnorr verification"
        );
    }

    #[test]
    fn test_pubkey_serialization_compressed_and_uncompressed() {
        // Create an uncompressed key.
        let key_data = [0x01; 32]; // Simple valid secret key
        let key_uncompressed = Key::new(&key_data, false).unwrap();
        let pubkey_uncompressed = key_uncompressed.get_pubkey();
        assert!(!pubkey_uncompressed.is_compressed());

        let serialized_uncompressed = pubkey_uncompressed.serialize();
        assert_eq!(serialized_uncompressed.len(), 65);
        assert_eq!(serialized_uncompressed[0], 0x04); // Uncompressed prefix

        // Round-trip uncompressed.
        let pk2 = PubKey::from_slice(&serialized_uncompressed).unwrap();
        assert!(!pk2.is_compressed());
        assert_eq!(pubkey_uncompressed.inner(), pk2.inner());

        // Create a compressed key from the same secret.
        let key_compressed = Key::new(&key_data, true).unwrap();
        let pubkey_compressed = key_compressed.get_pubkey();
        assert!(pubkey_compressed.is_compressed());

        let serialized_compressed = pubkey_compressed.serialize();
        assert_eq!(serialized_compressed.len(), 33);
        assert!(serialized_compressed[0] == 0x02 || serialized_compressed[0] == 0x03);

        // Both keys should represent the same point on the curve.
        assert_eq!(pubkey_compressed.inner(), pubkey_uncompressed.inner());
    }

    #[test]
    fn test_get_id_hash160() {
        // Use a known secret key to produce a deterministic pubkey.
        let key_data = [0x01; 32];
        let key = Key::new(&key_data, true).unwrap();
        let pubkey = key.get_pubkey();

        let id = pubkey.get_id();
        assert_eq!(id.len(), 20);

        // Verify manually: Hash160 of the compressed serialization.
        let expected = hash160(&pubkey.serialize());
        assert_eq!(id, expected);

        // The id should not be all zeros for a real key.
        assert!(id.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_xonly_pubkey_from_slice_roundtrip() {
        let key = Key::generate();
        let pubkey = key.get_pubkey();
        let xonly = XOnlyPubKey::from_pubkey(&pubkey);

        let serialized = xonly.serialize();
        assert_eq!(serialized.len(), 32);

        let xonly2 = XOnlyPubKey::from_slice(&serialized).unwrap();
        assert_eq!(xonly, xonly2);
    }

    #[test]
    fn test_key_new_invalid_data() {
        // All zeros is not a valid secret key.
        let result = Key::new(&[0u8; 32], true);
        assert!(result.is_err());

        // Too short.
        let result = Key::new(&[0x01; 16], true);
        assert!(result.is_err());
    }

    #[test]
    fn test_pubkey_from_slice_invalid() {
        // Invalid data should fail.
        let result = PubKey::from_slice(&[0x00; 33]);
        assert!(result.is_err());

        // Empty data should fail.
        let result = PubKey::from_slice(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_xonly_from_slice_invalid() {
        // All zeros is not a valid x-only pubkey.
        let result = XOnlyPubKey::from_slice(&[0u8; 32]);
        assert!(result.is_err());

        // Wrong length.
        let result = XOnlyPubKey::from_slice(&[0x01; 16]);
        assert!(result.is_err());
    }
}
