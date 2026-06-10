//! `frtun-pair` — stream-only client for the Subfrost mobile-as-signer
//! pair flow.
//!
//! Two NAT-bound peers (a Subfrost mobile wallet + a dapp CLI / browser)
//! rendezvous via a small bridge server using bech32 [`PeerName`]
//! addresses, then exchange application payloads over a bidirectional
//! [`PairStream`]. The bridge never sees plaintext — payloads are
//! end-to-end encrypted by the application layer (ChaCha20-Poly1305
//! over an X25519/HKDF-derived symKey, identical to the current
//! `subfrost-mobile-wc` envelope).
//!
//! Architecture:
//!
//! ```text
//!   ┌─────────┐                  ┌───────────────────┐                 ┌─────────┐
//!   │ Mobile  │ ── WSS frame ──▶ │ pair.subfrost.io  │ ──── WSS ─────▶ │  CLI    │
//!   │ wallet  │ ◀── WSS frame ── │ (frtun-pair-bridge│ ◀───  frame ──── │ (dapp)  │
//!   └─────────┘                  └───────────────────┘                 └─────────┘
//!                                  ↳ peer-name → connection map
//!                                  ↳ raw bytes glued verbatim
//! ```
//!
//! The CLI calls [`dial`] with the mobile's bech32 PeerName + the
//! shared bridge URL; the mobile calls [`listen`] on the same bridge
//! advertising its own PeerName. Both sides receive a
//! `AsyncRead+AsyncWrite` [`PairStream`] to use as they please —
//! typically: layer the `subfrost-mobile-wc` JSON-tagged Plaintext
//! envelopes on top.
//!
//! The crate is **transport-agnostic by default** (no `native`
//! feature). The only integration point is the [`BinaryDuplex`]
//! trait; callers wrap whatever WebSocket transport they're using
//! (native `tokio-tungstenite`, axum's `axum::extract::ws::WebSocket`,
//! a wasm browser WebSocket, the mobile-side `subfrost-mobile-transport`
//! tunnel, …) and pass it to [`handshake_dial`] / [`handshake_listen`].
//! Enable the `native` feature to opt-in to the bundled
//! `tokio-tungstenite` connector and the top-level [`dial`] /
//! [`listen`] helpers.
//!
//! The in-process [`Registry`] is also exposed so server-side hosts
//! (e.g. a `subfrost-mobile-api` axum router or the standalone
//! `frtun-pair-bridge` daemon) can embed the bridge logic without
//! pulling in the standalone server crate.
//!
//! ## Codec carve-out (tick-#618)
//!
//! The handshake protocol JSON + optional typed-frame layer + the
//! `BinaryDuplex` trait + spawn-free handshake entry points now live
//! in the sibling crate [`frtun_pair_codec`] so wasm consumers
//! (subfrost-wallet-web-sys) can speak the same Listen/Dial JSON
//! protocol without inheriting the tokio actor below. This crate
//! re-exports them so existing call sites compile unchanged, and adds
//! the [`PairStream`] tokio actor + [`Registry`] + native `dial` /
//! `listen` adapter on top.
//!
//! See `frtun-pair-bridge` for the standalone server-side daemon.

#![deny(rust_2018_idioms)]

pub mod handshake;
pub mod registry;
pub mod stream;

#[cfg(feature = "native")]
pub mod native;

// Re-export the codec surface from `frtun-pair-codec` so legacy call
// sites that import `frtun_pair::{ClientFrame, ServerFrame, codes,
// BinaryDuplex, FrameError, decode_frame, encode_frame,
// FRAME_TYPE_*}` keep compiling unchanged.
pub use frtun_pair_codec::{
    codes, BinaryDuplex, BoxedBinaryDuplex, ClientFrame, ServerFrame,
};

// `protocol` + `frame` re-export under the legacy module-path roots so
// `frtun_pair::protocol::ClientFrame` etc. resolve verbatim.
pub use frtun_pair_codec::protocol;
#[cfg(feature = "icmp")]
pub use frtun_pair_codec::frame;
#[cfg(feature = "icmp")]
pub use frtun_pair_codec::{
    decode_frame, encode_frame, FrameError, PingError, FRAME_TYPE_DATA, FRAME_TYPE_PING,
    FRAME_TYPE_PONG,
};

pub use handshake::{handshake_dial, handshake_listen, HandshakeError};
pub use registry::{ConnHandle, DialError, DialNotice, Registry};
pub use stream::PairStream;

#[cfg(feature = "native")]
pub use native::{dial, listen, NativeError};

// Identity helper re-export so consumers don't need to depend on
// frtun-identity directly. Gated behind a feature so the vendored
// copies (which don't ship frtun-identity in their workspace) can
// opt out and keep the historical surface.
#[cfg(feature = "identity-reexport")]
pub use frtun_identity::{KeyPair, PeerName};
