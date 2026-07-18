//! `frtun-pair-codec` — wasm-clean carve-out of the frtun-pair
//! handshake protocol.
//!
//! Lifted out of `frtun-pair` in tick-#618 so subfrost-wallet-web-sys
//! (the browser extension) can drive the same Listen/Dial →
//! Ready/Dialed/Incoming JSON protocol the native dapp CLI + axum
//! bridge already speak, without inheriting `frtun-pair`'s tokio-actor
//! `PairStream::spawn(...)` post-handshake driver.
//!
//! What's here:
//!   * [`protocol`] — `ClientFrame` / `ServerFrame` JSON wire enums +
//!     stable `codes`.
//!   * [`frame`] — optional typed-frame layer (`DATA` / `PING` /
//!     `PONG`); gated on the `icmp` feature.
//!   * [`stream::BinaryDuplex`] — async trait the handshake drives.
//!   * [`handshake_dial`] / [`handshake_listen`] — spawn-free handshake
//!     entry points; return the post-handshake byte stream + remote
//!     peer name. Caller wires the read/write loop appropriate for the
//!     target.
//!
//! What's NOT here (lives in the `frtun-pair` actor crate):
//!   * `PairStream` tokio-actor (AsyncRead+AsyncWrite over the duplex).
//!   * `Registry` (server-side peer routing table).
//!   * `native` (tokio-tungstenite client adapter).

#![deny(rust_2018_idioms)]

pub mod handshake;
pub mod protocol;
pub mod stream;

#[cfg(feature = "icmp")]
pub mod frame;

pub use handshake::{
    handshake_dial, handshake_listen, handshake_listen_with_token, HandshakeError,
};
pub use protocol::{codes, ClientFrame, ServerFrame};
pub use stream::{BinaryDuplex, BoxedBinaryDuplex};

#[cfg(feature = "icmp")]
pub use frame::{
    decode_frame, encode_frame, FrameError, PingError, FRAME_TYPE_DATA, FRAME_TYPE_PING,
    FRAME_TYPE_PONG,
};
