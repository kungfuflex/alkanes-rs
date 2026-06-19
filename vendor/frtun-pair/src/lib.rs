//! `frtun-pair` — vendored from subzero-rs (2026-05-24).
//!
//! Two NAT-bound peers (dapp CLI + Subfrost mobile wallet) rendezvous
//! via a single WSS endpoint by bech32 [`PeerName`] strings, then
//! exchange bytes verbatim through a bidirectional [`PairStream`].
//! The bridge never sees plaintext — payloads are end-to-end
//! encrypted by the application layer (existing `subfrost-mobile-wc`
//! ChaCha20-Poly1305 envelope).
//!
//! This crate is **transport-agnostic**. The only integration point
//! is the [`BinaryDuplex`] trait; callers wrap whatever WebSocket
//! transport they're using (native tokio-tungstenite, axum's
//! `axum::extract::ws::WebSocket`, a wasm browser WebSocket, the
//! mobile-side `subfrost-mobile-transport` tunnel, …) and pass it to
//! `handshake_dial` / `handshake_listen`. No tokio-tungstenite dep
//! here — that keeps the lib usable both inside subfrost-mobile-api's
//! axum router AND on the mobile side over the existing tunnel.
//!
//! On the server side, the bridge logic itself is in [`registry`] +
//! the consumer of the trait. See `subfrost-mobile-api`'s pair
//! handler for the canonical embed.

#![deny(rust_2018_idioms)]

pub mod handshake;
pub mod protocol;
pub mod registry;
pub mod stream;

#[cfg(feature = "client-native")]
pub mod client_native;

pub use handshake::{handshake_dial, handshake_listen, HandshakeError};
pub use protocol::{codes, ClientFrame, ServerFrame};
pub use registry::{ConnHandle, DialError, DialNotice, Registry};
pub use stream::{BinaryDuplex, PairStream};

#[cfg(feature = "client-native")]
pub use client_native::{dial, listen, NativeError};
