//! # `tlsfetch-events` — event + profiling primitives for the stack.
//!
//! Every transport and codec in tlsfetch carries a [`ProfilerHandle`]
//! and emits [`Event`]s at well-defined lifecycle points
//! (connection open, stream open / close / reset, byte counters,
//! datagram send / recv, codec-level milestones). Consumers attach
//! a real [`Profiler`] (e.g. [`Timeline`] for tests, a flamegraph
//! exporter in production) or use the default [`NoOpProfiler`] for
//! zero overhead.
//!
//! ## No-op cost
//!
//! The default [`NoOpProfiler`] resolves every `emit` to one virtual
//! call returning immediately. The compiler can't statically prove
//! it's a no-op (the handle is `Arc<dyn Profiler>` for runtime
//! pluggability), so emission sites still pay one indirect call.
//! In practice this is ~1 ns and dwarfed by the surrounding I/O.
//!
//! For paths where even that's too much, sites can guard with
//! [`ProfilerHandle::is_active`] which is a cheap branch on the
//! handle's `kind` field.
//!
//! ## Layering
//!
//! ```text
//!   tlsfetch-events                     leaf crate, no deps
//!   ├── used by tlsfetch-transport      Profiled<Inner> wrapper
//!   ├── used by tlsfetch-http1          Codec emits per-request events
//!   ├── used by tlsfetch-wt             Native + browser backends emit
//!   └── used by the consumer app       Holds the handle, renders UI
//! ```
//!
//! ## Layout of an emitted event
//!
//! ```ignore
//! ProfilerHandle::emit(Event::StreamWrite { conn_id, stream_id, bytes: 1024 })
//!   ↓
//! Timeline records (now_millis, Event)
//!   ↓
//! Test asserts ordering or consumer renders flamegraph.
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

use std::sync::Arc;

use web_time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Direction of a stream within a Connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDir {
    /// Bidirectional stream — both sides can read + write.
    Bi,
    /// Unidirectional stream — opener writes, peer reads.
    Uni,
}

/// Who initiated a stream open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Initiator {
    Local,
    Peer,
}

/// How a stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamCloseKind {
    /// Send side finished normally (FIN); recv side may still be open.
    Finish,
    /// Recv side hit EOF (peer FIN).
    PeerFinish,
    /// Local reset with the given application code.
    LocalReset { code: u32 },
    /// Peer reset with the given application code.
    PeerReset { code: u32 },
    /// Abandoned without explicit close (Stream dropped).
    Drop,
}

/// One event in a connection / stream / codec lifecycle. Variants
/// kept narrow on purpose — backends call sites should match exactly
/// one variant per emission. Consumer code matches on the discriminant
/// for filtering or renders the whole struct for flamegraphs.
///
/// `conn_id` / `stream_id` are raw `u64`s to keep this crate a leaf
/// (no dep on `tlsfetch-transport`'s `ConnId` / `StreamId` newtypes).
/// Consumers correlate by raw id.
#[derive(Debug, Clone)]
pub enum Event {
    // ----- connection lifecycle ------------------------------------------
    /// Connection establishment started (DNS / connect / handshake
    /// kickoff). Emitted by the client side; servers emit
    /// [`Event::Accepted`] on completion instead.
    Connecting {
        conn_id: u64,
        target: String,
    },
    /// Connection established. `alpn` is the negotiated ALPN protocol
    /// if the transport selects one (h3, h2, http/1.1, custom).
    Connected {
        conn_id: u64,
        peer_addr: Option<String>,
        alpn: Option<String>,
    },
    /// Server-side: a Listener accepted an inbound connection.
    Accepted {
        conn_id: u64,
        peer_addr: Option<String>,
    },
    /// Connection closed for any reason.
    ConnectionClosed {
        conn_id: u64,
        code: u32,
        reason: String,
    },
    /// Inbound connection rejected before any per-conn task
    /// spawned. Fires when a server-side cap (max_connections,
    /// rate limit, etc.) refused the accept. NO matching
    /// Accepted event was emitted for this connection — the
    /// reject path skips it entirely. `reason` is a short token
    /// the metrics layer labels its counter with (e.g.
    /// "max_connections").
    ConnectionDropped { reason: String },

    // ----- stream lifecycle ----------------------------------------------
    /// New stream allocated.
    StreamOpened {
        conn_id: u64,
        stream_id: u64,
        dir: StreamDir,
        initiator: Initiator,
    },
    /// Stream ended.
    StreamClosed {
        conn_id: u64,
        stream_id: u64,
        kind: StreamCloseKind,
    },

    // ----- byte counters -------------------------------------------------
    /// Wrote `bytes` to the send half of `stream_id`. Emitted in
    /// chunks; consumers sum to get the total.
    StreamWrite {
        conn_id: u64,
        stream_id: u64,
        bytes: u64,
    },
    /// Read `bytes` from the recv half of `stream_id`.
    StreamRead {
        conn_id: u64,
        stream_id: u64,
        bytes: u64,
    },

    // ----- datagrams -----------------------------------------------------
    DatagramSent {
        conn_id: u64,
        bytes: u64,
    },
    DatagramReceived {
        conn_id: u64,
        bytes: u64,
    },

    // ----- TLS / pinning -------------------------------------------------
    /// TLS handshake started.
    TlsHandshakeStart { conn_id: u64 },
    /// TLS handshake finished. `ja3` is the client's TLS fingerprint
    /// if the impl computed one; `pin_matched` is `Some(true|false)`
    /// when SPKI pinning was active.
    TlsHandshakeEnd {
        conn_id: u64,
        ja3: Option<String>,
        pin_matched: Option<bool>,
    },

    // ----- codec milestones ----------------------------------------------
    /// HTTP-style request started.
    RequestStart {
        conn_id: u64,
        stream_id: u64,
        method: String,
        path: String,
    },
    /// HTTP-style response headers received.
    ResponseHeaders {
        conn_id: u64,
        stream_id: u64,
        status: u16,
        content_length: Option<u64>,
    },
    /// HTTP-style response complete.
    RequestEnd {
        conn_id: u64,
        stream_id: u64,
        status: u16,
        bytes_sent: u64,
        bytes_received: u64,
    },

    // ----- catch-all -----------------------------------------------------
    /// Application-defined event. `data` is opaque to this crate.
    Custom {
        name: String,
        data: Option<String>,
    },
}

impl Event {
    /// Stable name suitable for grouping in a flamegraph or filter.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Connecting { .. } => "connecting",
            Event::Connected { .. } => "connected",
            Event::Accepted { .. } => "accepted",
            Event::ConnectionClosed { .. } => "connection_closed",
            Event::ConnectionDropped { .. } => "connection_dropped",
            Event::StreamOpened { .. } => "stream_opened",
            Event::StreamClosed { .. } => "stream_closed",
            Event::StreamWrite { .. } => "stream_write",
            Event::StreamRead { .. } => "stream_read",
            Event::DatagramSent { .. } => "datagram_sent",
            Event::DatagramReceived { .. } => "datagram_received",
            Event::TlsHandshakeStart { .. } => "tls_handshake_start",
            Event::TlsHandshakeEnd { .. } => "tls_handshake_end",
            Event::RequestStart { .. } => "request_start",
            Event::ResponseHeaders { .. } => "response_headers",
            Event::RequestEnd { .. } => "request_end",
            Event::Custom { .. } => "custom",
        }
    }
}

/// One [`Event`] tagged with the wall-clock instant it was emitted.
/// Stored by [`Timeline`]; consumed by tests + UI renderers.
#[derive(Debug, Clone)]
pub struct TimedEvent {
    pub at: SystemTime,
    pub event: Event,
}

impl TimedEvent {
    /// Milliseconds since UNIX epoch. Convenience for tests +
    /// log lines.
    pub fn millis(&self) -> u64 {
        self.at
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Profiler trait + handle
// ---------------------------------------------------------------------------

/// Sink for [`Event`]s. Implementors are typically reference-counted
/// and shared across many [`ProfilerHandle`] clones.
pub trait Profiler: Send + Sync {
    /// Record one event. Implementors MUST NOT block; emission is
    /// expected to be near-zero cost.
    fn emit(&self, at: SystemTime, event: Event);

    /// Hint that the consumer is or isn't actively recording. Lets
    /// hot paths skip emitting via [`ProfilerHandle::is_active`].
    /// Default implementation returns `true` — opt out by overriding.
    fn is_active(&self) -> bool {
        true
    }
}

/// Default no-op [`Profiler`]. Drops every event, reports inactive
/// so emission sites can branch out cheaply.
pub struct NoOpProfiler;

impl Profiler for NoOpProfiler {
    fn emit(&self, _at: SystemTime, _event: Event) {}
    fn is_active(&self) -> bool {
        false
    }
}

/// Cheap-to-clone handle every transport / codec carries. Callers
/// emit through [`Self::emit`]; the handle delegates to the wrapped
/// [`Profiler`].
#[derive(Clone)]
pub struct ProfilerHandle(Arc<dyn Profiler>);

impl ProfilerHandle {
    /// No-op handle — every `emit` returns immediately. Default
    /// for transports that aren't being profiled.
    pub fn noop() -> Self {
        Self(Arc::new(NoOpProfiler))
    }

    /// Wrap a custom [`Profiler`].
    pub fn new<P: Profiler + 'static>(p: P) -> Self {
        Self(Arc::new(p))
    }

    /// Wrap an already-`Arc`'d Profiler. Lets multiple handles share
    /// one timeline backing store.
    pub fn from_arc(p: Arc<dyn Profiler>) -> Self {
        Self(p)
    }

    /// Emit an event with `now()` as the timestamp.
    pub fn emit(&self, event: Event) {
        // Read clock once even when inactive so emission cost is
        // uniform and deterministic. The cost (a clock_gettime on
        // native, Date.now on wasm) is well under the dispatch
        // overhead of the inactive-check itself in benchmarks.
        let at = SystemTime::now();
        self.0.emit(at, event);
    }

    /// True iff the wrapped profiler reports active. Lets hot loops
    /// skip event construction:
    ///
    /// ```ignore
    /// if profiler.is_active() {
    ///     profiler.emit(Event::StreamWrite { conn_id, stream_id, bytes });
    /// }
    /// ```
    pub fn is_active(&self) -> bool {
        self.0.is_active()
    }
}

impl Default for ProfilerHandle {
    fn default() -> Self {
        Self::noop()
    }
}

impl std::fmt::Debug for ProfilerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfilerHandle")
            .field("active", &self.is_active())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Timeline — in-memory collector for tests + debug overlays.
// ---------------------------------------------------------------------------

/// In-memory [`Profiler`] that appends every event to a Mutex-guarded
/// `Vec`. Used by:
///
/// - the `tlsfetch-transport` conformance harness to assert backends
///   emit events at expected lifecycle points,
/// - a consumer app session debug overlay to render a live
///   flamegraph of the current sync,
/// - the `tlsfetch-cli --trace` flag to dump a request's timeline.
///
/// Construct via [`Self::new`], hand a clone of the resulting
/// `Arc<Timeline>` to [`ProfilerHandle::from_arc`], and inspect
/// emissions with [`Self::snapshot`].
pub struct Timeline {
    events: parking_lot::Mutex<Vec<TimedEvent>>,
}

impl Timeline {
    /// Empty timeline.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: parking_lot::Mutex::new(Vec::new()),
        })
    }

    /// Cheap clone of the entire event log. Returns a `Vec<TimedEvent>`
    /// rather than borrowing so callers can drop the lock immediately.
    pub fn snapshot(&self) -> Vec<TimedEvent> {
        self.events.lock().clone()
    }

    /// Number of events recorded so far.
    pub fn len(&self) -> usize {
        self.events.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.lock().is_empty()
    }

    /// Drop everything. Useful to reset between test phases.
    pub fn clear(&self) {
        self.events.lock().clear();
    }

    /// Filter helper — return only events whose name matches.
    pub fn filter_name(&self, name: &str) -> Vec<TimedEvent> {
        self.events
            .lock()
            .iter()
            .filter(|t| t.event.name() == name)
            .cloned()
            .collect()
    }

    /// Render an ASCII flamegraph-style trace. One line per event;
    /// columns are: relative-millis, event name, key fields. Used by
    /// the `tlsfetch-cli --trace` flag and test failure messages.
    pub fn render(&self) -> String {
        let snap = self.snapshot();
        if snap.is_empty() {
            return String::from("(empty timeline)");
        }
        let t0 = snap[0].millis();
        let mut out = String::new();
        for ev in snap {
            let dt = ev.millis().saturating_sub(t0);
            out.push_str(&format!("{:>6}ms  {}", dt, ev.event.name()));
            // tack on a few discriminating fields per variant
            match &ev.event {
                Event::Connecting { conn_id, target } => {
                    out.push_str(&format!("  conn={} target={}", conn_id, target));
                }
                Event::Connected { conn_id, alpn, .. } => {
                    out.push_str(&format!(
                        "  conn={} alpn={}",
                        conn_id,
                        alpn.as_deref().unwrap_or("-")
                    ));
                }
                Event::StreamOpened {
                    conn_id,
                    stream_id,
                    dir,
                    initiator,
                } => {
                    out.push_str(&format!(
                        "  conn={} sid={} {:?} {:?}",
                        conn_id, stream_id, dir, initiator
                    ));
                }
                Event::StreamClosed {
                    conn_id,
                    stream_id,
                    kind,
                } => {
                    out.push_str(&format!(
                        "  conn={} sid={} {:?}",
                        conn_id, stream_id, kind
                    ));
                }
                Event::StreamWrite {
                    conn_id,
                    stream_id,
                    bytes,
                }
                | Event::StreamRead {
                    conn_id,
                    stream_id,
                    bytes,
                } => {
                    out.push_str(&format!(
                        "  conn={} sid={} bytes={}",
                        conn_id, stream_id, bytes
                    ));
                }
                Event::RequestStart {
                    method, path, ..
                } => {
                    out.push_str(&format!("  {} {}", method, path));
                }
                Event::ResponseHeaders { status, .. } => {
                    out.push_str(&format!("  status={}", status));
                }
                Event::RequestEnd {
                    status,
                    bytes_sent,
                    bytes_received,
                    ..
                } => {
                    out.push_str(&format!(
                        "  status={} sent={} recv={}",
                        status, bytes_sent, bytes_received
                    ));
                }
                _ => {}
            }
            out.push('\n');
        }
        out
    }
}

impl Profiler for Timeline {
    fn emit(&self, at: SystemTime, event: Event) {
        self.events.lock().push(TimedEvent { at, event });
    }
    fn is_active(&self) -> bool {
        true
    }
}

/// Convenience: hand a fresh [`Timeline`] + matching [`ProfilerHandle`]
/// in one call.
///
/// ```
/// # use tlsfetch_events::{timeline, Event};
/// let (tl, handle) = timeline();
/// handle.emit(Event::Custom { name: "tick".into(), data: None });
/// assert_eq!(tl.len(), 1);
/// ```
pub fn timeline() -> (Arc<Timeline>, ProfilerHandle) {
    let tl = Timeline::new();
    let handle = ProfilerHandle::from_arc(tl.clone() as Arc<dyn Profiler>);
    (tl, handle)
}
