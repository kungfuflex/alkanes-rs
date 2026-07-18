//! Subfrost mobile-as-signer protocol (custom WalletConnect).
//!
//! Three responsibilities:
//!
//!   * `crypto` — X25519 ECDH + HKDF-SHA256 + ChaCha20-Poly1305.
//!     Both Rust (mobile) and TS (`subfrost-app/lib/wc/crypto.ts`)
//!     produce byte-identical outputs given the same inputs;
//!     `tests/crypto.rs` cross-checks the test vectors.
//!
//!   * `pairing` — parse `subfrost://wc/<topic>?key=...` URIs into
//!     PendingPairing, generate the mobile-side X25519 keypair,
//!     derive the shared symmetric key.
//!
//!   * `wire` — serde shapes for the JSON envelopes the relay
//!     ferries. The relay never sees plaintext; envelopes are
//!     opaque `{ciphertext, nonce}` blobs.

pub mod crypto;
pub mod pairing;
pub mod wire;

#[cfg(feature = "relay-client")]
pub mod relay;
#[cfg(feature = "relay-client")]
pub mod signer;

pub use crypto::{ecdh_derive, encrypt, decrypt, gen_keypair, NONCE_LEN, KEY_LEN};
pub use pairing::{parse_pairing_uri, PendingPairing, PairingError};
pub use wire::{Plaintext, RequestEnvelope, ResponseEnvelope};
