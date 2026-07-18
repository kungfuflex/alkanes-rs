//! # `tlsfetch-transport` — the contract every transport satisfies
//!
//! This crate defines the trait surface that every byte / datagram
//! transport in the tlsfetch stack obeys. Codecs (HTTP/1.1, HTTP/2,
//! WebSocket, gRPC, IP-over-anything, raw RPC) are written against
//! these traits exactly once, and they work over every transport
//! that satisfies them, on every target (native, wasm32, in-process
//! loopback, anything addable later).
//!
//! Tunneling = composition. There is no special "tunnel" trait; if
//! protocol X is "a codec over a Stream" and transport Y "produces
//! Streams", then "X over Y" is a one-liner. WebTransport-tunneled
//! HTTP/1.1, WSS-tunneled raw IP, HTTP/2-tunneled WebTransport — all
//! the same shape: pick a Stream out of a Connection, run a codec on
//! it.
//!
//! ## Layered model
//!
//! ```text
//!   codecs           Http1Codec  Http2Codec  WebSocketCodec  IpPacketCodec ...
//!   ─────────────────────────────────────────────────────────────────────────
//!   transport API    trait Stream  trait Connection  trait Listener
//!                    + adapters: FromStream, SingleStream, Profiled, Pinned
//!   ─────────────────────────────────────────────────────────────────────────
//!   impls            InMemory   tlsfetch-tcp   tlsfetch-wt   tlsfetch-ws ...
//! ```
//!
//! Codecs never know which transport they're running on. Transports
//! never know which codec rides on top. Pin / event / integrity
//! concerns live in orthogonal wrapper types (`Pinned<S>`,
//! `Profiled<C>`, `IntegrityGated<L>`) that compose freely.
//!
//! ## The contract
//!
//! Every concrete impl of [`Stream`], [`Connection`], or [`Listener`]
//! MUST satisfy the contract that follows. The conformance test suite
//! in `tests/contract.rs` asserts every clause; an impl that passes
//! the suite is a conforming impl, full stop.
//!
//! ### Stream contract
//!
//! 1. **Order preservation.** Bytes written to a `Stream`'s send half
//!    are delivered to the peer's recv half in the same order. No
//!    reordering, no duplication, no gaps until EOF or reset.
//! 2. **Reliable delivery.** Once `poll_close_send` returns `Ready(Ok)`
//!    and the connection is still healthy, the peer is guaranteed to
//!    eventually see every byte and then EOF.
//! 3. **Half-close.** `poll_close_send` finalizes the send direction
//!    only. The recv direction stays open; the peer can still send.
//!    Calling it twice is OK; second call is a no-op.
//! 4. **Reset.** `poll_reset` aborts both directions. Pending writes
//!    are discarded. The peer's reads surface
//!    [`TransportError::StreamReset`] with the supplied error code.
//!    Calling reset after close-send is OK; it just terminates the
//!    recv side.
//! 5. **Drop semantics.** Dropping a `Stream` without explicit close
//!    or reset SHOULD reset(0). It MUST NOT silently leave the stream
//!    open on the wire.
//! 6. **Backpressure.** `poll_write` MUST return `Pending` when the
//!    underlying flow-control window is full. It MUST NOT buffer
//!    unbounded.
//!
//! ### Connection contract
//!
//! 7. **Concurrent stream open.** `open_bi` and `open_uni` may be
//!    called concurrently from many tasks. Each call gets a unique
//!    stream id.
//! 8. **Concurrent accept.** `accept_bi` and `accept_uni` likewise
//!    queue incoming streams; calls retrieve them in arrival order.
//! 9. **Stream / datagram independence.** A backed-up datagram path
//!    MUST NOT block stream progress, and vice versa.
//! 10. **Close drains.** `close()` triggers an orderly shutdown:
//!     all open streams transition to reset(code), pending datagrams
//!     are dropped, `closed()` resolves with the supplied code.
//! 11. **Idempotent close.** Multiple `close()` calls are no-ops
//!     after the first. `closed()` is multi-await safe.
//! 12. **Stable conn id.** `conn_id()` returns the same value for
//!     the connection's whole lifetime, suitable for profiler
//!     correlation.
//!
//! ### Listener contract
//!
//! 13. **One acceptor.** `accept` is called from a single task at a
//!     time. Concurrent calls are not required to be supported.
//! 14. **Backlog.** Implementations SHOULD queue at least one
//!     pending connection so a slow accept loop doesn't drop peers.
//! 15. **Close stops accept.** After `close()`, subsequent
//!     `accept()` returns
//!     [`TransportError::Closed`] with the supplied code.
//!
//! ### Cross-cutting
//!
//! 16. **Error conversion.** Every backend-specific error MUST map
//!     to one of the [`TransportError`] variants; consumers get a
//!     single error surface.
//! 17. **No `Send` outside native.** Trait bounds use [`MaybeSend`]
//!     / [`MaybeSync`], which are `Send`/`Sync` on non-wasm and
//!     unbounded on `wasm32-unknown-unknown` (single-threaded JS
//!     runtime).
//! 18. **Events are optional.** Every method is callable with a
//!     no-op profiler. Setting a real profiler MUST NOT change
//!     observable semantics, only emit timing data.
//!
//! ## What this crate does NOT do
//!
//! - **TLS.** Handled by `tlsfetch-tls` / `tlsfetch-pin` as a
//!   wrapper that turns a `Stream` into a TLS-encrypted `Stream`.
//! - **Codecs.** HTTP/1.1, HTTP/2, WebSocket, gRPC live in their
//!   own crates and consume the traits here.
//! - **Concrete transports.** TCP, UDP, WebTransport, WebSocket,
//!   in-memory loopback all live in their own crates and implement
//!   the traits here.
//! - **Pinning logic.** `tlsfetch-pin` provides
//!   `Pinned<S: Stream>` that wraps any Stream with TLS+pin
//!   verification.
//! - **Integrity gates.** `tlsfetch-integrity` provides
//!   `IntegrityGated<L: Listener>` middleware.

#![cfg_attr(docsrs, feature(doc_cfg))]

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::future::BoxFuture;
use futures_io::{AsyncRead, AsyncWrite};
use thiserror::Error;

// ---------------------------------------------------------------------------
// MaybeSend / MaybeSync — `Send`+`Sync` on native, unbounded on wasm32.
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}

#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync + ?Sized> MaybeSync for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSync for T {}

/// Boxed future flavor matching the target. Native gets `Send`-bound
/// futures so streams move between executor threads; wasm gets
/// `LocalBoxFuture` because JS values are `!Send`.
#[cfg(not(target_arch = "wasm32"))]
pub type TransportFuture<'a, T> = BoxFuture<'a, T>;
#[cfg(target_arch = "wasm32")]
pub type TransportFuture<'a, T> = futures_core::future::LocalBoxFuture<'a, T>;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Single error surface shared by every transport in the stack.
/// Backend-specific errors stringify into one of these variants at
/// the trait boundary.
#[derive(Debug, Error)]
pub enum TransportError {
    /// Initial handshake / connect attempt failed.
    #[error("connect: {0}")]
    Connect(String),
    /// Connection was closed cleanly with the given code + reason.
    #[error("closed: code={code} reason={reason}")]
    Closed { code: u32, reason: String },
    /// Stream was reset by either side. Stream-only; doesn't bring
    /// down the connection.
    #[error("stream reset: code={code}")]
    StreamReset { code: u32 },
    /// Underlying I/O failure (socket, JS WebTransport, etc.).
    #[error("io: {0}")]
    Io(String),
    /// Cert pin mismatch / TLS verifier rejection. Surfaced via
    /// `tlsfetch-pin`'s `Pinned<S>` wrapper.
    #[error("pin: {0}")]
    Pin(String),
    /// Build-time integrity hash check failed. Surfaced via
    /// `tlsfetch-integrity`'s `IntegrityGated<L>` middleware.
    #[error("integrity: {0}")]
    Integrity(String),
    /// Datagram-path error — payload exceeds negotiated MTU,
    /// datagrams disabled by peer, queue full, etc. Distinct from
    /// `StreamReset` because datagrams have no stream concept.
    #[error("datagram: {0}")]
    Datagram(String),
    /// Backend-specific error that doesn't map cleanly to the above.
    #[error("transport: {0}")]
    Other(String),
}

pub type TransportResult<T> = Result<T, TransportError>;

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

/// Stable identifier for one [`Connection`] within a process. Issued
/// by the implementor; opaque to consumers. Used by `Profiled<_>` to
/// correlate events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnId(pub u64);

/// Stable identifier for one [`Stream`] within its owning Connection.
/// `(ConnId, StreamId)` is globally unique within a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(pub u64);

// ---------------------------------------------------------------------------
// Stream
// ---------------------------------------------------------------------------

/// A bidirectional, ordered, reliable byte channel. Supersets
/// [`AsyncRead`] + [`AsyncWrite`] so it interops with hyper,
/// tokio-tungstenite, wasm-streams, etc., zero-cost.
///
/// See the crate-level docs for the **Stream contract** every impl
/// MUST satisfy.
pub trait Stream: AsyncRead + AsyncWrite + Unpin + MaybeSend {
    /// Stream id within its owning connection.
    fn stream_id(&self) -> StreamId;

    /// Half-close the send direction (FIN). After Ready(Ok), peer's
    /// reads return EOF once pending data has drained. Send-side
    /// writes after this return [`TransportError::Closed`].
    fn poll_close_send(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<TransportResult<()>>;

    /// Abruptly reset both directions with the given app error code.
    /// Returns immediately; the underlying signal may take one RTT
    /// to land on the peer.
    fn reset(self: Pin<&mut Self>, code: u32);

    /// Best-effort priority hint. Honored on transports that
    /// expose it (HTTP/2, HTTP/3); ignored otherwise.
    fn set_priority(self: Pin<&mut Self>, _priority: u8) {}
}

/// Convenience extension trait — async sugar over the poll_-based
/// methods on [`Stream`]. Auto-impl for every Stream impl.
pub trait StreamExt: Stream {
    /// `await`able variant of [`Stream::poll_close_send`].
    fn close_send(&mut self) -> CloseSend<'_, Self>
    where
        Self: Unpin + Sized,
    {
        CloseSend { stream: self }
    }
}

impl<T: Stream + ?Sized> StreamExt for T {}

/// Future returned by [`StreamExt::close_send`].
pub struct CloseSend<'a, S: ?Sized> {
    stream: &'a mut S,
}

impl<'a, S: Stream + Unpin + ?Sized> std::future::Future for CloseSend<'a, S> {
    type Output = TransportResult<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        Pin::new(&mut *this.stream).poll_close_send(cx)
    }
}

// Boxed-stream type aliases. Used by the trait-object Connection
// surface so consumers don't carry the concrete impl through
// generics. Send-bound on native, unbounded on wasm32.

#[cfg(not(target_arch = "wasm32"))]
pub type DynStream = Pin<Box<dyn Stream + Send + Unpin>>;
#[cfg(target_arch = "wasm32")]
pub type DynStream = Pin<Box<dyn Stream + Unpin>>;

#[cfg(not(target_arch = "wasm32"))]
pub type DynSendStream = Pin<Box<dyn AsyncWrite + Send + Unpin>>;
#[cfg(target_arch = "wasm32")]
pub type DynSendStream = Pin<Box<dyn AsyncWrite + Unpin>>;

#[cfg(not(target_arch = "wasm32"))]
pub type DynRecvStream = Pin<Box<dyn AsyncRead + Send + Unpin>>;
#[cfg(target_arch = "wasm32")]
pub type DynRecvStream = Pin<Box<dyn AsyncRead + Unpin>>;

/// Backend-supplied reset hook for a [`BiStream`]. Called once with
/// the application error code when the stream is reset (explicitly
/// via [`BiStream::reset`] or implicitly via Drop). Boxed `Fn` rather
/// than `FnOnce` so we can guard double-calls inside [`BiStream`]'s
/// own state and keep the hook trivially cloneable for backends that
/// share state.
#[cfg(not(target_arch = "wasm32"))]
pub type ResetHook = Box<dyn Fn(u32) + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type ResetHook = Box<dyn Fn(u32)>;

/// Bidirectional stream pair returned by `open_bi` / `accept_bi`.
/// Send + recv halves are split so codecs can send/recv concurrently.
///
/// Reset is exposed at the BiStream level rather than per-half: most
/// transports tear down both directions atomically (QUIC RST_STREAM,
/// HTTP/2 RST_STREAM, etc.) and codecs that want a one-sided
/// terminator just drop the half they don't need.
///
/// ## Drop semantics
///
/// - If the send half was closed cleanly (`poll_close` returned
///   `Ready(Ok)`), Drop is a no-op — graceful shutdown, peer reads
///   to EOF normally.
/// - Otherwise Drop fires `reset_hook(0)` — implements stream
///   contract clause 5 (drop without explicit close = reset).
pub struct BiStream {
    pub send: DynSendStream,
    pub recv: DynRecvStream,
    pub stream_id: StreamId,
    /// Backend hook; `None` after [`Self::reset`] runs (manual reset)
    /// or after Drop fires it.
    reset_hook: Option<ResetHook>,
    /// Set by the [`TrackedSendStream`] wrapper around `send` when
    /// `poll_close` returns `Ready(Ok)`. If true at Drop time, the
    /// reset hook is suppressed — the peer already saw a clean
    /// half-close.
    send_clean_closed: Arc<AtomicBool>,
}

impl BiStream {
    /// Construct a BiStream from the parts a backend produced. Most
    /// users should never call this; backends invoke it from their
    /// `open_bi` / `accept_bi` implementations.
    pub fn new(
        send: DynSendStream,
        recv: DynRecvStream,
        stream_id: StreamId,
        reset_hook: ResetHook,
    ) -> Self {
        let send_clean_closed = Arc::new(AtomicBool::new(false));
        let tracked = TrackedSendStream {
            inner: send,
            clean_close: send_clean_closed.clone(),
        };
        Self {
            send: Box::pin(tracked),
            recv,
            stream_id,
            reset_hook: Some(reset_hook),
            send_clean_closed,
        }
    }

    /// Construct a BiStream whose reset is a no-op. Used by transports
    /// that don't model stream-level reset (raw TCP+TLS, in-memory
    /// loopback when reset is not yet wired).
    pub fn no_reset(send: DynSendStream, recv: DynRecvStream, stream_id: StreamId) -> Self {
        Self::new(send, recv, stream_id, Box::new(|_| {}))
    }

    /// Abort both directions with the given application error code.
    /// Idempotent: a second call is a no-op.
    pub fn reset(&mut self, code: u32) {
        if let Some(hook) = self.reset_hook.take() {
            hook(code);
        }
    }

    /// Collapse the bidi pair into a single `AsyncRead + AsyncWrite`
    /// duplex over the same stream. Useful for codecs that take a
    /// unified stream — most prominently TLS (rustls /
    /// `tlsfetch_pin::connect_pinned_over_stream`) and any
    /// HTTP/1.1 / framing layer that wants one object.
    ///
    /// Like [`Self::into_halves`] this disarms the Drop-fires-reset
    /// behavior; lifetime is owned by the returned [`JoinedDuplex`].
    /// On Drop the joined duplex closes the send half and drops the
    /// recv half — same shape as a `TcpStream` shutdown.
    pub fn into_joined(self) -> JoinedDuplex {
        let (send, recv, stream_id) = self.into_halves();
        JoinedDuplex {
            send,
            recv,
            stream_id,
        }
    }

    /// Split into the three constituent parts, *disarming* the
    /// Drop-fires-reset behavior. Use this when you're handing one
    /// half off and intend to manage stream lifetime explicitly via
    /// the halves themselves (e.g. extracting the send half for a
    /// unidirectional stream and discarding the recv half).
    pub fn into_halves(mut self) -> (DynSendStream, DynRecvStream, StreamId) {
        // Take the hook so the upcoming Drop doesn't reset(0).
        self.reset_hook.take();
        // Bring fields out via std::mem::replace + a sentinel. We
        // can't move out of `self` directly because of the Drop impl;
        // ManuallyDrop is the conventional escape hatch.
        let mut me = std::mem::ManuallyDrop::new(self);
        // SAFETY: we own `me`, no other reference exists, and the
        // ManuallyDrop suppresses the eventual Drop. The reads move
        // each field out exactly once.
        unsafe {
            let send = std::ptr::read(&me.send);
            let recv = std::ptr::read(&me.recv);
            let stream_id = me.stream_id;
            // Defensive: drop `reset_hook` (already None) so its
            // backing memory is freed when `me` goes out of scope.
            // ManuallyDrop's contents are otherwise leaked.
            std::ptr::drop_in_place(&mut me.reset_hook);
            (send, recv, stream_id)
        }
    }
}

impl Drop for BiStream {
    fn drop(&mut self) {
        // Stream contract clause 5: dropping without explicit close
        // or reset SHOULD reset(0). If the user closed the send half
        // cleanly (TrackedSendStream flipped the flag), we skip —
        // graceful shutdown, peer's read_to_end resolves with EOF.
        if self.send_clean_closed.load(std::sync::atomic::Ordering::Acquire) {
            // Discard the hook without firing.
            self.reset_hook.take();
            return;
        }
        if let Some(hook) = self.reset_hook.take() {
            hook(0);
        }
    }
}

// ---------------------------------------------------------------------------
// TrackedSendStream — internal wrapper that flips a shared flag when
// poll_close completes. Lets BiStream::Drop tell graceful close from
// abrupt drop without needing the user to think about it.
// ---------------------------------------------------------------------------

use std::sync::atomic::AtomicBool;

struct TrackedSendStream {
    inner: DynSendStream,
    clean_close: Arc<AtomicBool>,
}

impl AsyncWrite for TrackedSendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.inner.as_mut().poll_write(cx, buf)
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.inner.as_mut().poll_flush(cx)
    }
    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let r = this.inner.as_mut().poll_close(cx);
        if let std::task::Poll::Ready(Ok(())) = &r {
            this.clean_close
                .store(true, std::sync::atomic::Ordering::Release);
        }
        r
    }
}

impl std::fmt::Debug for BiStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BiStream")
            .field("stream_id", &self.stream_id)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// JoinedDuplex — unify the send + recv halves back into one
// AsyncRead+AsyncWrite for codecs that want a single stream object.
// Built specifically because tlsfetch-pin's `connect_pinned_over_stream`
// is generic over `S: AsyncRead + AsyncWrite + Unpin` — feeding it a
// WebTransport bidi means re-joining the halves the wasm-streams
// adapter handed us split.
// ---------------------------------------------------------------------------

/// Unified `AsyncRead + AsyncWrite` view over a [`BiStream`] pair.
/// Constructed via [`BiStream::into_joined`].
pub struct JoinedDuplex {
    send: DynSendStream,
    recv: DynRecvStream,
    pub stream_id: StreamId,
}

impl AsyncRead for JoinedDuplex {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.recv.as_mut().poll_read(cx, buf)
    }
}

impl AsyncWrite for JoinedDuplex {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.send.as_mut().poll_write(cx, buf)
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.send.as_mut().poll_flush(cx)
    }
    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        this.send.as_mut().poll_close(cx)
    }
}

impl std::fmt::Debug for JoinedDuplex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinedDuplex")
            .field("stream_id", &self.stream_id)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Datagrams
// ---------------------------------------------------------------------------

/// Datagram interface for transports that support it (WebTransport,
/// QUIC, raw UDP). Stream + datagram paths are independent — a
/// backed-up stream MUST NOT block datagrams and vice versa.
pub struct DatagramHandle {
    pub sink: std::sync::Arc<dyn DatagramSink>,
    pub source: std::sync::Arc<dyn DatagramSource>,
    pub max_size: usize,
}

pub trait DatagramSink: MaybeSend + MaybeSync {
    /// Try to send one datagram synchronously. Returns
    /// [`TransportError::Other`] if the send queue is full
    /// (transport-specific behavior).
    fn try_send(&self, payload: Bytes) -> TransportResult<()>;
}

pub trait DatagramSource: MaybeSend + MaybeSync {
    /// Wait for the next datagram from the peer. Resolves with the
    /// payload, or an error if the datagram path closes.
    fn recv(&self) -> TransportFuture<'_, TransportResult<Bytes>>;
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/// A live session capable of carrying many concurrent streams +
/// (optionally) a datagram path. WebTransport, QUIC, HTTP/2 client
/// connection, in-process loopback all surface as Connection.
///
/// See the crate-level docs for the **Connection contract** every
/// impl MUST satisfy.
pub trait Connection: MaybeSend + MaybeSync {
    /// Stable id for profiler correlation. Constant for the
    /// connection's lifetime.
    fn conn_id(&self) -> ConnId;

    /// Open a new bidirectional stream. Resolves once the stream id
    /// is allocated and the wire-level init frames have been sent.
    fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>>;

    /// Accept the next inbound bidirectional stream from the peer.
    /// Calls queue; subsequent calls retrieve in arrival order.
    fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>>;

    /// Open a unidirectional stream. Returns the send half; the peer
    /// receives the recv half via `accept_uni`.
    fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>>;

    /// Accept the next inbound unidirectional stream. Returns the
    /// recv half.
    fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>>;

    /// Datagram path for this connection. Returns `None` on transports
    /// that don't carry datagrams (raw TCP, single-stream adapters).
    fn datagrams(&self) -> Option<DatagramHandle>;

    /// Close the connection with the given app error code + reason.
    /// Idempotent.
    fn close(&self, code: u32, reason: &[u8]);

    /// Resolves when the connection is closed for any reason. Safe
    /// to await from multiple tasks.
    fn closed(&self) -> TransportFuture<'_, TransportError>;

    /// Remote peer's address for transports that know it.
    /// Default `None` because not every Connection has a notion
    /// of "peer addr" (e.g. in-memory loopback). Native TLS
    /// listeners override to surface the inbound TCP accept's
    /// SocketAddr. Used by tlsd's ProxyService to populate
    /// `Event::Accepted { peer_addr }` for metrics / access logs.
    fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        None
    }

    /// Negotiated ALPN protocol bytes (e.g. `b"http/1.1"`,
    /// `b"h2"`) for transports that ran an ALPN-aware handshake.
    /// Default `None` — plain TCP / in-memory loopback have no
    /// ALPN. Lets ProxyService route based on the codec the
    /// client negotiated; today it only logs a warning when the
    /// codec isn't H1 since H2/H3 inbound is a follow-up.
    fn alpn(&self) -> Option<&[u8]> {
        None
    }
}

// ---------------------------------------------------------------------------
// Listener
// ---------------------------------------------------------------------------

/// Accepts incoming Connections from a bound endpoint.
///
/// See the crate-level docs for the **Listener contract**.
pub trait Listener: MaybeSend {
    /// Bind address as a transport-specific string (e.g.
    /// "127.0.0.1:443" for native, "/local/loopback/0" for an
    /// in-memory transport). `None` if the impl can't synthesize one.
    fn local_addr(&self) -> Option<String>;

    /// Wait for the next inbound connection.
    fn accept(&mut self) -> TransportFuture<'_, TransportResult<Box<dyn Connection>>>;

    /// Stop accepting new connections. Subsequent `accept()` returns
    /// [`TransportError::Closed`].
    fn close(&mut self);
}

/// Listener counterpart of [`ProfiledConnection`]. Wraps any inner
/// [`Listener`] and emits an `Accepted` event for each connection
/// the inner listener returns successfully. Each accepted connection
/// is itself wrapped in a `ProfiledConnection` so its stream-open
/// events flow through the same profiler handle.
///
/// Native-only — see [`ProfiledConnection`].
#[cfg(not(target_arch = "wasm32"))]
pub struct ProfiledListener<L: Listener> {
    inner: L,
    profiler: tlsfetch_events::ProfilerHandle,
}

#[cfg(not(target_arch = "wasm32"))]
impl<L: Listener> ProfiledListener<L> {
    pub fn new(inner: L, profiler: tlsfetch_events::ProfilerHandle) -> Self {
        Self { inner, profiler }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<L: Listener> Listener for ProfiledListener<L> {
    fn local_addr(&self) -> Option<String> {
        self.inner.local_addr()
    }

    fn accept(&mut self) -> TransportFuture<'_, TransportResult<Box<dyn Connection>>> {
        let profiler = self.profiler.clone();
        let fut = self.inner.accept();
        use futures_util::FutureExt;
        async move {
            let conn = fut.await?;
            let conn_id = conn.conn_id().0;
            profiler.emit(tlsfetch_events::Event::Accepted {
                conn_id,
                peer_addr: None,
            });
            // Wrap so subsequent open_bi/accept_bi/close emit through
            // the same profiler.
            let boxed: Box<dyn Connection> = Box::new(BoxedProfiled {
                inner: conn,
                profiler,
                conn_id: ConnId(conn_id),
            });
            Ok(boxed)
        }
        .boxed()
    }

    fn close(&mut self) {
        self.inner.close();
    }
}

/// Internal wrapper that satisfies `Connection` over a `Box<dyn Connection>`
/// while emitting events. Same shape as ProfiledConnection<C> but the
/// inner is type-erased so it can flow through `Box<dyn Listener>`.
#[cfg(not(target_arch = "wasm32"))]
struct BoxedProfiled {
    inner: Box<dyn Connection>,
    profiler: tlsfetch_events::ProfilerHandle,
    conn_id: ConnId,
}

#[cfg(not(target_arch = "wasm32"))]
impl Connection for BoxedProfiled {
    fn conn_id(&self) -> ConnId {
        self.conn_id
    }

    fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.open_bi();
        use futures_util::FutureExt;
        async move {
            let bi = fut.await?;
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: bi.stream_id.0,
                dir: tlsfetch_events::StreamDir::Bi,
                initiator: tlsfetch_events::Initiator::Local,
            });
            Ok(bi)
        }
        .boxed()
    }

    fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.accept_bi();
        use futures_util::FutureExt;
        async move {
            let bi = fut.await?;
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: bi.stream_id.0,
                dir: tlsfetch_events::StreamDir::Bi,
                initiator: tlsfetch_events::Initiator::Peer,
            });
            Ok(bi)
        }
        .boxed()
    }

    fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>> {
        self.inner.open_uni()
    }

    fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>> {
        self.inner.accept_uni()
    }

    fn datagrams(&self) -> Option<DatagramHandle> {
        self.inner.datagrams()
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.profiler
            .emit(tlsfetch_events::Event::ConnectionClosed {
                conn_id: self.conn_id.0,
                code,
                reason: String::from_utf8_lossy(reason).to_string(),
            });
        self.inner.close(code, reason);
    }

    fn closed(&self) -> TransportFuture<'_, TransportError> {
        self.inner.closed()
    }
}

// ---------------------------------------------------------------------------
// Adapters (signatures only; bodies land in M2)
// ---------------------------------------------------------------------------

/// Wrap a raw `AsyncRead + AsyncWrite` (e.g. a TCP stream, an
/// in-memory pair, a tls-tlsfetch handshake result) as a degenerate
/// [`Connection`] with one bidi stream and no datagrams. Useful when
/// running a Connection-shaped codec over a single-stream transport.
pub struct FromStream<S> {
    _inner: std::marker::PhantomData<S>,
}

/// Pick one bidi stream out of an existing [`Connection`] and
/// present THAT as a degenerate Connection (with no further streams
/// or datagrams). Useful when running a Connection-shaped codec on
/// a sub-channel of a multiplexed transport.
pub struct SingleStream<C> {
    _inner: std::marker::PhantomData<C>,
}

/// Instrument any [`Connection`] with per-method event emission.
///
/// Wraps an inner Connection + a [`tlsfetch_events::ProfilerHandle`]
/// and emits lifecycle events at well-defined points:
///
/// - `StreamOpened { Local }` after a successful `open_bi` / `open_uni`.
/// - `StreamOpened { Peer }`  after a successful `accept_bi` / `accept_uni`.
/// - `ConnectionClosed { code, reason }` synchronously inside `close()`.
///
/// Stream-level byte counters (StreamWrite / StreamRead) are NOT
/// emitted — instrumenting every poll would add overhead even when
/// the profiler is no-op. Codecs (HTTP/1.1, gRPC) emit byte-level
/// events at their own granularity instead.
///
/// Native-only: wasm32 builds don't compile this wrapper because
/// `BoxFuture<Send>` is incompatible with the JS-bound futures the
/// browser backend produces. Browser consumers use the codec-layer
/// profiler emission instead (see `tlsfetch-relay::RelayConfig::with_profiler`).
#[cfg(not(target_arch = "wasm32"))]
pub struct ProfiledConnection<C: Connection> {
    inner: C,
    profiler: tlsfetch_events::ProfilerHandle,
    conn_id: ConnId,
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: Connection> ProfiledConnection<C> {
    /// Wrap `inner` and emit lifecycle events through `profiler`.
    /// `conn_id()` is computed once and kept stable across calls
    /// regardless of the inner impl.
    pub fn new(inner: C, profiler: tlsfetch_events::ProfilerHandle) -> Self {
        let conn_id = inner.conn_id();
        Self {
            inner,
            profiler,
            conn_id,
        }
    }

    /// Borrow the wrapped Connection. Useful for tests that need to
    /// observe the inner directly.
    pub fn inner(&self) -> &C {
        &self.inner
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: Connection> Connection for ProfiledConnection<C> {
    fn conn_id(&self) -> ConnId {
        self.conn_id
    }

    fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.open_bi();
        use futures_util::FutureExt;
        async move {
            let bi = fut.await?;
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: bi.stream_id.0,
                dir: tlsfetch_events::StreamDir::Bi,
                initiator: tlsfetch_events::Initiator::Local,
            });
            Ok(bi)
        }
        .boxed()
    }

    fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.accept_bi();
        use futures_util::FutureExt;
        async move {
            let bi = fut.await?;
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: bi.stream_id.0,
                dir: tlsfetch_events::StreamDir::Bi,
                initiator: tlsfetch_events::Initiator::Peer,
            });
            Ok(bi)
        }
        .boxed()
    }

    fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.open_uni();
        use futures_util::FutureExt;
        async move {
            let s = fut.await?;
            // No stable per-uni stream id here; pass 0 — consumers
            // can correlate via wall-clock if they need to.
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: 0,
                dir: tlsfetch_events::StreamDir::Uni,
                initiator: tlsfetch_events::Initiator::Local,
            });
            Ok(s)
        }
        .boxed()
    }

    fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>> {
        let profiler = self.profiler.clone();
        let conn_id = self.conn_id.0;
        let fut = self.inner.accept_uni();
        use futures_util::FutureExt;
        async move {
            let s = fut.await?;
            profiler.emit(tlsfetch_events::Event::StreamOpened {
                conn_id,
                stream_id: 0,
                dir: tlsfetch_events::StreamDir::Uni,
                initiator: tlsfetch_events::Initiator::Peer,
            });
            Ok(s)
        }
        .boxed()
    }

    fn datagrams(&self) -> Option<DatagramHandle> {
        self.inner.datagrams()
    }

    fn close(&self, code: u32, reason: &[u8]) {
        self.profiler
            .emit(tlsfetch_events::Event::ConnectionClosed {
                conn_id: self.conn_id.0,
                code,
                reason: String::from_utf8_lossy(reason).to_string(),
            });
        self.inner.close(code, reason);
    }

    fn closed(&self) -> TransportFuture<'_, TransportError> {
        self.inner.closed()
    }
}

/// Wrap a raw [`Stream`] with TLS termination + SPKI pin
/// verification. Bodies in M3 once `tlsfetch-tls` lands; the type
/// alias is reserved here so consumer crates can refer to it stably.
pub struct Pinned<Inner> {
    _inner: std::marker::PhantomData<Inner>,
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

/// Conformance harness exposed when the `testing` feature is on.
/// Concrete transport impls invoke `tlsfetch_transport_contract!`
/// from this module to inherit the whole suite.
#[cfg(feature = "testing")]
pub mod testing;

/// Async glue commonly used alongside this crate. Re-exported so
/// consumers don't need to track `futures-core` / `futures-io`
/// version skew themselves.
pub mod prelude {
    pub use super::{
        BiStream, Connection, ConnId, DatagramHandle, DatagramSink, DatagramSource, DynRecvStream,
        DynSendStream, DynStream, Listener, MaybeSend, MaybeSync, ResetHook, Stream, StreamExt,
        StreamId, TransportError, TransportFuture, TransportResult,
    };
    pub use futures_io::{AsyncRead, AsyncWrite};
}

#[cfg(test)]
mod connection_peer_addr_tests {
    use super::*;
    use std::net::SocketAddr;

    /// Connection impl that overrides peer_addr to return a
    /// known value. Confirms the trait override dispatches
    /// through a `&dyn Connection`.
    struct WithPeer {
        peer: SocketAddr,
    }
    impl Connection for WithPeer {
        fn conn_id(&self) -> ConnId {
            ConnId(0)
        }
        fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>> {
            unimplemented!("test-only")
        }
        fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>> {
            unimplemented!("test-only")
        }
        fn datagrams(&self) -> Option<DatagramHandle> {
            None
        }
        fn close(&self, _code: u32, _reason: &[u8]) {}
        fn closed(&self) -> TransportFuture<'_, TransportError> {
            unimplemented!("test-only")
        }
        fn peer_addr(&self) -> Option<SocketAddr> {
            Some(self.peer)
        }
    }

    /// Connection impl that doesn't override peer_addr —
    /// confirms the default `fn peer_addr -> None` arm.
    struct DefaultPeer;
    impl Connection for DefaultPeer {
        fn conn_id(&self) -> ConnId {
            ConnId(0)
        }
        fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>> {
            unimplemented!("test-only")
        }
        fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>> {
            unimplemented!("test-only")
        }
        fn datagrams(&self) -> Option<DatagramHandle> {
            None
        }
        fn close(&self, _code: u32, _reason: &[u8]) {}
        fn closed(&self) -> TransportFuture<'_, TransportError> {
            unimplemented!("test-only")
        }
        // no peer_addr override → default returns None
    }

    #[test]
    fn peer_addr_override_routes_through_dyn_dispatch() {
        let addr: SocketAddr = "127.0.0.1:65000".parse().unwrap();
        let c: Box<dyn Connection> = Box::new(WithPeer { peer: addr });
        assert_eq!(c.peer_addr(), Some(addr));
    }

    #[test]
    fn peer_addr_default_is_none() {
        let c: Box<dyn Connection> = Box::new(DefaultPeer);
        assert_eq!(c.peer_addr(), None);
    }

    /// Same pattern for the alpn override. WithAlpn returns
    /// Some(b"h2"); DefaultPeer (no alpn override) returns None.
    struct WithAlpn {
        alpn: Vec<u8>,
    }
    impl Connection for WithAlpn {
        fn conn_id(&self) -> ConnId {
            ConnId(0)
        }
        fn open_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn accept_bi(&self) -> TransportFuture<'_, TransportResult<BiStream>> {
            unimplemented!("test-only")
        }
        fn open_uni(&self) -> TransportFuture<'_, TransportResult<DynSendStream>> {
            unimplemented!("test-only")
        }
        fn accept_uni(&self) -> TransportFuture<'_, TransportResult<DynRecvStream>> {
            unimplemented!("test-only")
        }
        fn datagrams(&self) -> Option<DatagramHandle> {
            None
        }
        fn close(&self, _code: u32, _reason: &[u8]) {}
        fn closed(&self) -> TransportFuture<'_, TransportError> {
            unimplemented!("test-only")
        }
        fn alpn(&self) -> Option<&[u8]> {
            Some(&self.alpn)
        }
    }

    #[test]
    fn alpn_override_routes_through_dyn_dispatch() {
        let c: Box<dyn Connection> = Box::new(WithAlpn {
            alpn: b"h2".to_vec(),
        });
        assert_eq!(c.alpn(), Some(b"h2".as_ref()));
    }

    #[test]
    fn alpn_default_is_none() {
        let c: Box<dyn Connection> = Box::new(DefaultPeer);
        assert_eq!(c.alpn(), None);
    }
}
