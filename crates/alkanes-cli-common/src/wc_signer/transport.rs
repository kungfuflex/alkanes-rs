//! Native impl of `WalletTransport`.
//!
//! Trait + error type now live in `wc_signer_core::transport`; this
//! file is the native-only impl. Native impl dials WSS via
//! [`tlsfetch_ws::WsClient`] and pokes `/v1/pair-wake` via the HTTP
//! twin endpoint. The frtun-pair handshake (Listen/Dial →
//! Ready/Dialed/Incoming) is transport-agnostic — we plug the
//! WsClient in via a small `BinaryDuplex` adapter so the wire shape
//! (Text frames + canonical op names) stays identical to the wasm +
//! axum-side impls.
//!
//! Wasm impl lives in `subfrost-wallet-web-sys::wc_signer` —
//! different I/O primitive (browser `WebSocket` + `fetch`) but same
//! trait shape (`wc_signer_core::WalletTransport`).
//!
//! The pair-wake POST goes through the in-tree
//! `wc_signer::http_async` helper (tokio + tokio-rustls), NOT reqwest.
//! The helper is a near-duplicate of the canonical at
//! `~/frtun/crates/frtun-push-proxy/src/http_async.rs`; collapse when
//! tlsfetch drops its rustls patch (see http_async.rs file-level doc).

// Re-export the core trait + error type so existing
// `alkanes_cli_common::wc_signer::transport::{TransportError,
// WalletPairStream, WalletTransport}` import paths keep compiling.
#[cfg(feature = "wc-signer")]
pub use wc_signer_core::transport::{TransportError, WalletPairStream, WalletTransport};

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
pub use native::NativeTransport;

#[cfg(all(feature = "wc-signer-native", not(target_arch = "wasm32")))]
mod native {
    use async_trait::async_trait;
    use bytes::Bytes;
    use frtun_pair::stream::BinaryDuplex;
    use std::io;
    use std::time::Duration;
    use tlsfetch_ws::{
        TransportError as WsTransportError, WsClient, WsConnectOptions, WsMessage,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::time::{interval, Interval, MissedTickBehavior};
    use wc_signer_core::transport::{
        TransportError, WalletPairStream, WalletTransport,
    };

    /// Period between application-level WS Ping frames sent from a
    /// listening CLI to keep the connection alive against upstream
    /// idle timeouts.
    ///
    /// Cloudflare's free-tier WSS idle timeout sits around 100s. A
    /// 30s ping cadence stays comfortably under that ceiling while
    /// being long enough to not flood the relay with traffic when
    /// the listener may park for hours waiting for a phone to dial.
    /// The remote peer's auto-pong reply is filtered as a `Pong`
    /// frame in the recv loop and dropped.
    const KEEPALIVE_PING_INTERVAL: Duration = Duration::from_secs(30);

    pub struct NativeTransport;

    impl NativeTransport {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for NativeTransport {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Bridge between `tlsfetch_ws::WsClient` and frtun-pair's
    /// transport-agnostic [`BinaryDuplex`] trait.
    ///
    /// The handshake (`handshake_dial` / `handshake_listen`) drives
    /// this through `send_text` / `recv_text`; once the handshake
    /// completes, frtun-pair's `PairStream::spawn` actor uses the
    /// `send_binary` / `recv_binary` halves to relay raw bytes between
    /// peers. Ping/Pong are auto-handled by tokio-tungstenite under
    /// the hood — we just filter them out of the recv loop.
    ///
    /// **Wire-shape pin**: frtun-pair sends `{"op":"listen"|"dial",...}`
    /// as a single WS Text frame (see `vendor/frtun-pair/src/handshake.rs`
    /// and `protocol.rs`). The server expects Text — anything else is
    /// a hard-reset by Cloudflare, which surfaces as "Connection reset
    /// without closing handshake". We never originate Binary frames
    /// before the handshake completes.
    ///
    /// **Keepalive (2026-06-09 tick)**: a `keepalive` `Interval` ticks
    /// every [`KEEPALIVE_PING_INTERVAL`]; on each tick we send a WS
    /// Ping frame so Cloudflare's WSS idle timer (~100s on the free
    /// tier) sees fresh traffic and doesn't RST the listener while
    /// it parks waiting for the phone to dial in. The ticker only
    /// gets polled inside `recv_binary` / `recv_text` (the actor's
    /// blocking edges) which is sufficient because those are exactly
    /// the long-idle edges; the send paths are by definition not
    /// idle. The ticker is `Some` only after we mark this duplex as
    /// keepalive-active via [`WsClientDuplex::with_keepalive`]; the
    /// dial path (short-lived, completes well under 100s) leaves it
    /// as `None` to avoid spurious pings.
    struct WsClientDuplex {
        inner: WsClient,
        keepalive: Option<Interval>,
    }

    impl WsClientDuplex {
        fn new(inner: WsClient) -> Self {
            Self { inner, keepalive: None }
        }

        /// Enable a 30s WS Ping keepalive on receive-side blocking
        /// edges. Mutates self for fluent construction in `listen`.
        fn with_keepalive(mut self) -> Self {
            let mut i = interval(KEEPALIVE_PING_INTERVAL);
            // First tick fires immediately; skip it so we don't ping
            // before the handshake-completion settles, and recover
            // from any stall by burning the missed ticks instead of
            // bursting them.
            i.set_missed_tick_behavior(MissedTickBehavior::Delay);
            self.keepalive = Some(i);
            self
        }

        /// Pull the next frame from `inner.recv`, sending a Ping on
        /// each keepalive tick. Returns the raw `WsMessage` or `None`
        /// on clean close. Pings/Pongs are surfaced verbatim — the
        /// duplex impl's recv_binary/recv_text loop filters them.
        async fn recv_with_keepalive(
            &mut self,
        ) -> Result<Option<WsMessage>, WsTransportError>
        {
            // The keepalive arm is only constructed when the listener
            // path enabled it. When it's `None`, the future never
            // resolves so the select! collapses to a plain recv.
            //
            // We carve out a sub-block so the recv future + ping
            // future can borrow `inner` and `keepalive` disjointly
            // through split borrows.
            let Self { inner, keepalive } = self;
            loop {
                match keepalive {
                    Some(ticker) => {
                        tokio::select! {
                            biased;
                            r = inner.recv() => return r,
                            _ = ticker.tick() => {
                                // Best-effort ping; if the ping
                                // itself errors, fall back to a
                                // plain recv so the caller still
                                // gets the underlying error surfaced
                                // via the next recv.
                                if let Err(e) = inner.send_ping(Bytes::new()).await {
                                    log::debug!(
                                        "wc_signer keepalive ping failed: {e}"
                                    );
                                    return Err(e);
                                }
                                // Re-enter the loop to keep blocking
                                // on inbound frames.
                                continue;
                            }
                        }
                    }
                    None => return inner.recv().await,
                }
            }
        }
    }

    #[async_trait]
    impl BinaryDuplex for WsClientDuplex {
        async fn send_binary(&mut self, data: Bytes) -> io::Result<()> {
            self.inner
                .send_binary(data)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))
        }

        async fn recv_binary(&mut self) -> io::Result<Option<Bytes>> {
            loop {
                match self.recv_with_keepalive().await {
                    Ok(Some(WsMessage::Binary(b))) => return Ok(Some(b)),
                    Ok(Some(WsMessage::Ping(_))) | Ok(Some(WsMessage::Pong(_))) => continue,
                    Ok(Some(WsMessage::Text(_))) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unexpected text frame post-handshake",
                        ))
                    }
                    Ok(None) => return Ok(None),
                    Err(e) => {
                        return Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
                    }
                }
            }
        }

        async fn send_text(&mut self, text: String) -> io::Result<()> {
            // The frtun-pair bridge expects the handshake as JSON Text
            // frames; sending Binary triggers a Cloudflare RST. We
            // added `send_text` to the vendored tlsfetch-ws's WsClient
            // in tick 2; backport to upstream when convenient.
            self.inner
                .send_text(&text)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))
        }

        async fn recv_text(&mut self) -> io::Result<Option<String>> {
            loop {
                match self.recv_with_keepalive().await {
                    Ok(Some(WsMessage::Text(s))) => return Ok(Some(s)),
                    Ok(Some(WsMessage::Ping(_))) | Ok(Some(WsMessage::Pong(_))) => continue,
                    Ok(Some(WsMessage::Binary(_))) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unexpected binary frame during handshake",
                        ))
                    }
                    Ok(None) => return Ok(None),
                    Err(e) => {
                        return Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
                    }
                }
            }
        }

        async fn close(&mut self) -> io::Result<()> {
            let _ = self.inner.close(1000, "bye").await;
            Ok(())
        }
    }

    async fn dial_ws(bridge_url: &str) -> Result<WsClientDuplex, TransportError> {
        let ws = WsClient::connect(bridge_url, &WsConnectOptions::default())
            .await
            .map_err(|e| TransportError::Dial(format!("ws connect: {e}")))?;
        Ok(WsClientDuplex::new(ws))
    }

    /// Adapter wrapping `frtun_pair::PairStream` as a `WalletPairStream`.
    struct PairStreamAdapter {
        inner: frtun_pair::PairStream,
        remote: String,
    }

    #[async_trait]
    impl WalletPairStream for PairStreamAdapter {
        async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            self.inner
                .write_all(bytes)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))
        }

        async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError> {
            let mut buf = vec![0u8; 65536];
            let n = self
                .inner
                .read(&mut buf)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
            if n == 0 {
                return Err(TransportError::Io("eof".into()));
            }
            buf.truncate(n);
            Ok(buf)
        }

        async fn close(&mut self) {
            let _ = self.inner.shutdown().await;
        }

        fn remote_peer(&self) -> &str {
            &self.remote
        }

        #[cfg(feature = "icmp")]
        async fn ping(&mut self, timeout: Duration) -> Result<Duration, TransportError> {
            self.inner
                .ping(timeout)
                .await
                .map_err(|e| TransportError::Ping(e.to_string()))
        }
    }

    #[async_trait]
    impl WalletTransport for NativeTransport {
        async fn dial(
            &self,
            bridge_url: &str,
            self_peer: &str,
            remote_peer: &str,
        ) -> Result<Box<dyn WalletPairStream>, TransportError> {
            let duplex = dial_ws(bridge_url).await?;
            let stream = frtun_pair::handshake_dial(duplex, self_peer, remote_peer)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("peer_not_found") {
                        TransportError::PeerNotFound
                    } else {
                        TransportError::Dial(msg)
                    }
                })?;
            let remote = stream.remote_peer().to_string();
            Ok(Box::new(PairStreamAdapter { inner: stream, remote }))
        }

        async fn listen(
            &self,
            bridge_url: &str,
            self_peer: &str,
        ) -> Result<Box<dyn WalletPairStream>, TransportError> {
            // Enable a 30s WS Ping keepalive on the listener duplex.
            // Cloudflare's free-tier WSS idle timeout (~100s) was
            // closing the CLI's listen connection before the phone
            // dialed in; mobile-side dial then hit a deregistered
            // listener and returned `peer_not_registered`, surfaced
            // on the CLI as "Connection reset without closing
            // handshake". The dial path is short-lived (well under
            // 100s) so it stays keepalive-off — we don't want
            // spurious ping traffic on a one-shot handshake.
            let duplex = dial_ws(bridge_url).await?.with_keepalive();
            let stream = frtun_pair::handshake_listen(duplex, self_peer)
                .await
                .map_err(|e| TransportError::Listen(e.to_string()))?;
            let remote = stream.remote_peer().to_string();
            Ok(Box::new(PairStreamAdapter { inner: stream, remote }))
        }

        async fn pair_wake(
            &self,
            bridge_url: &str,
            peer: &str,
        ) -> Result<bool, TransportError> {
            // The pair-wake POST used to go through reqwest. We swapped
            // it for the in-tree `http_async` helper (tokio + tokio-rustls
            // + webpki-roots, hand-rolled H1 codec) to keep the
            // wc-signer-native subgraph off reqwest. The helper is a
            // sibling to the canonical `tlsfetch_common::client_async`
            // and to `~/frtun/crates/frtun-push-proxy/src/http_async.rs`;
            // see `wc_signer/http_async.rs` for the consolidation TODO.
            use crate::wc_signer::http_async::send_async;

            let http_base = wss_to_https_base(bridge_url);
            let url = format!("{}/v1/pair-wake", http_base);
            let body = serde_json::json!({ "peer": peer });
            let body_bytes = serde_json::to_vec(&body)
                .map_err(|e| TransportError::Http(format!("encode wake body: {e}")))?;
            let resp = send_async(
                &url,
                "POST",
                &[("Content-Type", "application/json")],
                Some(&body_bytes),
            )
            .await
            .map_err(|e| TransportError::Http(e.to_string()))?;
            let json: serde_json::Value = serde_json::from_slice(&resp.body)
                .map_err(|e| TransportError::Http(format!("decode wake response: {e}")))?;
            if !(200..300).contains(&resp.status) {
                return Err(TransportError::Http(format!(
                    "{} {}",
                    resp.status,
                    json.get("reason").and_then(|v| v.as_str()).unwrap_or("")
                )));
            }
            Ok(json
                .get("delivered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
        }
    }

    /// Strip `/v1/pair` suffix + flip scheme. Mirrors the TS helper.
    fn wss_to_https_base(wss: &str) -> String {
        let base = if let Some(rest) = wss.strip_prefix("wss://") {
            format!("https://{}", rest)
        } else if let Some(rest) = wss.strip_prefix("ws://") {
            format!("http://{}", rest)
        } else {
            wss.to_string()
        };
        // Drop /v1/pair if present.
        if let Some(idx) = base.find("/v1/pair") {
            base[..idx].to_string()
        } else {
            base.trim_end_matches('/').to_string()
        }
    }

    #[cfg(test)]
    mod native_tests {
        use super::*;

        #[test]
        fn wss_url_to_http_base() {
            assert_eq!(
                wss_to_https_base("wss://wss-tls.subfrost.io/v1/pair"),
                "https://wss-tls.subfrost.io"
            );
            assert_eq!(
                wss_to_https_base("ws://127.0.0.1:18801/v1/pair"),
                "http://127.0.0.1:18801"
            );
        }
    }
}
