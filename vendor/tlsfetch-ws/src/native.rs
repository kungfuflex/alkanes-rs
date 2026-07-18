//! Native backend over `tokio-tungstenite` with `rustls-tls-webpki-roots`.
//!
//! See the crate-level docs for the TLS-engine-agnostic invariant —
//! this module never names `rustls` directly; cert verification rides
//! tokio-tungstenite's built-in stack.

use crate::{WsConnectOptions, WsMessage};

use bytes::Bytes;
use futures_util::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::time::Duration;
use tlsfetch_transport::TransportError;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{
        error::Error as TungsteniteError,
        http::Request,
        protocol::{frame::coding::CloseCode, CloseFrame, Message},
    },
    MaybeTlsStream, WebSocketStream,
};

type Stream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Sink = SplitSink<Stream, Message>;
type Source = SplitStream<Stream>;

/// Async WebSocket client. Single-task ownership — the split sink and
/// stream halves are held internally and serialized through the public
/// `send_binary` / `recv` / `close` API.
pub struct WsClient {
    sink: Sink,
    source: Source,
}

impl WsClient {
    /// Dial a `ws://` or `wss://` endpoint.
    ///
    /// The url scheme determines whether the underlying connection is
    /// plaintext or TLS-wrapped (via tokio-tungstenite's
    /// `rustls-tls-webpki-roots` feature). Subprotocols and headers
    /// are folded into the handshake request; `connect_timeout` is
    /// applied via `tokio::time::timeout` over the whole TCP + TLS +
    /// WS sequence.
    pub async fn connect(
        url: &str,
        opts: &WsConnectOptions,
    ) -> Result<Self, TransportError> {
        // Tick 1 deferred: real pin verification + insecure-skip.
        // Log a TODO and proceed with webpki-roots verification.
        if opts.insecure {
            log::warn!(
                "tlsfetch-ws: insecure=true requested but not yet \
                 wired (tick 1) — proceeding with default verification"
            );
        }
        if opts.spki_pins.is_some() {
            log::warn!(
                "tlsfetch-ws: spki_pins set but pinning not yet \
                 wired (tick 1) — proceeding with default verification"
            );
        }

        // Build the handshake Request so we can attach subprotocols +
        // custom headers. tokio-tungstenite 0.21 takes anything that
        // implements `IntoClientRequest`; the http crate's
        // `Request<()>` does, and the builder lets us set headers
        // before handing it off.
        let parsed = url::Url::parse(url).map_err(|e| {
            TransportError::Connect(format!("invalid url: {e}"))
        })?;
        let host = parsed
            .host_str()
            .ok_or_else(|| TransportError::Connect("url has no host".into()))?;
        let port = parsed.port_or_known_default().unwrap_or_else(|| {
            match parsed.scheme() {
                "wss" => 443,
                _ => 80,
            }
        });

        let mut builder = Request::builder()
            .method("GET")
            .uri(url)
            // Required handshake headers tokio-tungstenite would set
            // for a bare string `connect_async`. Setting them here
            // mirrors that path; tokio-tungstenite overrides
            // `Sec-WebSocket-Key` per-handshake anyway.
            .header("Host", format!("{host}:{port}"))
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            );

        if !opts.subprotocols.is_empty() {
            builder = builder.header(
                "Sec-WebSocket-Protocol",
                opts.subprotocols.join(", "),
            );
        }

        for (name, value) in &opts.headers {
            // Skip reserved handshake headers (case-insensitive) — let
            // tokio-tungstenite emit the canonical values. Lets
            // callers pass through Authorization / User-Agent / etc.
            // without accidentally fighting the handshake.
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "host"
                    | "upgrade"
                    | "connection"
                    | "sec-websocket-key"
                    | "sec-websocket-version"
                    | "sec-websocket-protocol"
            ) {
                continue;
            }
            builder = builder.header(name.as_str(), value.as_str());
        }

        let request = builder
            .body(())
            .map_err(|e| TransportError::Connect(format!("request build: {e}")))?;

        let connect_fut = tokio_tungstenite::connect_async(request);

        let (ws, _resp) = match opts.connect_timeout {
            Some(d) => tokio::time::timeout(d, connect_fut)
                .await
                .map_err(|_| TransportError::Connect("connect timeout".into()))?
                .map_err(map_tungstenite_err)?,
            None => connect_fut.await.map_err(map_tungstenite_err)?,
        };

        let (sink, source) = ws.split();
        Ok(Self { sink, source })
    }

    /// Send one binary frame.
    pub async fn send_binary(
        &mut self,
        payload: Bytes,
    ) -> Result<(), TransportError> {
        // tungstenite 0.21 takes `Vec<u8>` — copy out of the Bytes
        // since we don't own it. (Frame encode will copy again
        // internally; bytes::Bytes doesn't help past this point.)
        self.sink
            .send(Message::Binary(payload.to_vec()))
            .await
            .map_err(map_tungstenite_err)
    }

    /// Send one text frame.
    ///
    /// Added in alkanes-rs-develop's vendor of tlsfetch-ws (tick 2 of
    /// the TLSFETCH-WS migration) because the frtun-pair `/v1/pair`
    /// rendezvous protocol exchanges its handshake as JSON Text
    /// frames; sending those as Binary triggers a Cloudflare RST
    /// instead of a clean WS-layer close. Backport to upstream
    /// `~/tlsfetch` when convenient — it's a strict superset of the
    /// existing send_binary API.
    pub async fn send_text(
        &mut self,
        text: &str,
    ) -> Result<(), TransportError> {
        self.sink
            .send(Message::Text(text.to_owned()))
            .await
            .map_err(map_tungstenite_err)
    }

    /// Send a WebSocket Ping frame (opcode 0x9).
    ///
    /// Used by callers that need to defeat upstream idle timeouts
    /// (e.g. Cloudflare's free-tier ~100s WSS idle close) on a
    /// listen-only connection that may park silently for minutes
    /// before the peer dials in. The payload is normally empty
    /// (`Bytes::new()`) but the WebSocket spec allows up to 125
    /// bytes — tungstenite will reject larger payloads.
    ///
    /// The remote peer's auto-pong reply is surfaced through
    /// [`Self::recv`] as [`WsMessage::Pong`]; callers can ignore it.
    pub async fn send_ping(
        &mut self,
        payload: Bytes,
    ) -> Result<(), TransportError> {
        self.sink
            .send(Message::Ping(payload.to_vec()))
            .await
            .map_err(map_tungstenite_err)
    }

    /// Pull the next frame. Returns `Ok(None)` on clean peer close.
    ///
    /// Ping / Pong frames are surfaced as-is; auto-pong is handled by
    /// the underlying tungstenite WebSocket. Close frames resolve to
    /// `Ok(None)`. Raw `Message::Frame` is filtered out (tungstenite
    /// only emits it when read-as-frames is configured, which it
    /// isn't here).
    pub async fn recv(
        &mut self,
    ) -> Result<Option<WsMessage>, TransportError> {
        loop {
            match self.source.next().await {
                Some(Ok(Message::Binary(b))) => {
                    return Ok(Some(WsMessage::Binary(Bytes::from(b))))
                }
                Some(Ok(Message::Text(s))) => {
                    return Ok(Some(WsMessage::Text(s)))
                }
                Some(Ok(Message::Ping(p))) => {
                    return Ok(Some(WsMessage::Ping(Bytes::from(p))))
                }
                Some(Ok(Message::Pong(p))) => {
                    return Ok(Some(WsMessage::Pong(Bytes::from(p))))
                }
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(Message::Frame(_))) => continue,
                Some(Err(e)) => return Err(map_tungstenite_err(e)),
                None => return Ok(None),
            }
        }
    }

    /// Send a Close frame and flush.
    ///
    /// `code` is the RFC 6455 close code (e.g. `1000` for normal,
    /// `1001` for going-away). `reason` is a short UTF-8 string the
    /// peer may surface in its own close handling. The underlying
    /// codec sends + flushes the close; the caller may drop the
    /// `WsClient` afterwards.
    pub async fn close(
        &mut self,
        code: u16,
        reason: &str,
    ) -> Result<(), TransportError> {
        let frame = CloseFrame {
            code: CloseCode::from(code),
            reason: std::borrow::Cow::Owned(reason.to_owned()),
        };
        self.sink
            .send(Message::Close(Some(frame)))
            .await
            .map_err(map_tungstenite_err)?;
        // Best-effort flush; on a half-closed transport this may
        // return Err which we surface to the caller.
        self.sink.flush().await.map_err(map_tungstenite_err)
    }
}

/// Map tungstenite's error surface onto [`TransportError`].
///
/// We try to keep the variants distinguishable:
/// - `Url` / `HttpFormat` / `Http` errors → `Connect`
/// - `Tls` / `Io` → `Io`
/// - `ConnectionClosed` / `AlreadyClosed` → `Closed`
/// - everything else → `Other`
fn map_tungstenite_err(e: TungsteniteError) -> TransportError {
    match e {
        TungsteniteError::Url(u) => {
            TransportError::Connect(format!("url: {u}"))
        }
        TungsteniteError::HttpFormat(h) => {
            TransportError::Connect(format!("http format: {h}"))
        }
        TungsteniteError::Http(resp) => TransportError::Connect(format!(
            "http: status={}",
            resp.status().as_u16()
        )),
        TungsteniteError::Tls(t) => {
            TransportError::Io(format!("tls: {t}"))
        }
        TungsteniteError::Io(io) => TransportError::Io(io.to_string()),
        TungsteniteError::ConnectionClosed => TransportError::Closed {
            code: 1000,
            reason: "connection closed".into(),
        },
        TungsteniteError::AlreadyClosed => TransportError::Closed {
            code: 1000,
            reason: "already closed".into(),
        },
        other => TransportError::Other(format!("ws: {other}")),
    }
}

/// Re-exported so consumers that want to detect the
/// "0 connect timeout" sentinel can compare a Duration value.
#[allow(dead_code)]
const _: Option<Duration> = None;
