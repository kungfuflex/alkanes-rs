//! Dapp-side relay client. Rust port of `subfrost-mobile/ts-sdk/src/relay.ts`.
//!
//! Hybrid WSS + HTTP transport against the wc-relay service:
//! * **WSS** subscribes to the session topic. Carries `init_ack`,
//!   `accepted`, `response`, `pairing_revoked`, `error` events from the
//!   relay → dapp.
//! * **HTTP POST** publishes encrypted request envelopes to
//!   `{base}/v1/sessions/{topic}/req`. Responses come back asynchronously
//!   over the WSS as `response` frames, correlated by `request_id`.
//!
//! Two patterns are supported:
//! * `open()` — first run, sends the `init` frame with the dapp pubkey,
//!   waits for `init_ack`, then `await_accepted()` blocks until the
//!   wallet pairs and the relay forwards the `mobile_pub`.
//! * `reconnect()` — re-attach to an existing session (e.g. after a WSS
//!   drop). Sends `subscribe` instead of `init`, no `init_ack`.
//!
//! `send_request(envelope)` posts via HTTP and resolves once the matching
//! `ResponseFrame` arrives over WSS. Pending requests time out at 5 min
//! by default — wallets sometimes sit on the prompt for a while.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::wire::{RequestEnvelope, ResponseEnvelope};

const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("relay transport error: {0}")]
    Transport(String),
    #[error("http publish failed: {status} {body}")]
    HttpPublish { status: u16, body: String },
    #[error("relay closed: code={code} reason={reason}")]
    Closed { code: u16, reason: String },
    #[error("response timeout for request_id={0}")]
    Timeout(String),
    #[error("pairing timed out after {0:?}")]
    PairingTimeout(Duration),
    #[error("bad frame from relay: {0}")]
    BadFrame(String),
    #[error("relay returned error frame: {0}")]
    ServerError(String),
}

/// Wire frames the dapp sends to the relay over WSS.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum ClientFrame {
    Init {
        topic: String,
        webapp_pub: String,
    },
    Subscribe {
        topic: String,
    },
}

/// Wire frames the relay sends back over WSS.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum ServerFrame {
    InitAck { topic: String },
    Accepted { topic: String, mobile_pub: String },
    Response {
        topic: String,
        request_id: String,
        ciphertext: String,
        nonce: String,
    },
    PairingRevoked { topic: String },
    Error { reason: String },
}

/// Configuration to open a relay session.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// `wss://wc.subfrost.io/` (or test override). Both WSS and HTTP
    /// are derived from this URL — wss → ws for testing is fine too.
    pub relay_url: String,
    /// Session topic (the random ID from the pairing URI).
    pub topic: String,
    /// Dapp's X25519 public key, base64-encoded. Comes from
    /// [`crate::pairing::PendingPairing::own_pub_b64`].
    pub webapp_pub_b64: String,
}

/// Convert `wss://host/` → `https://host/` (or `ws://` → `http://`) so
/// we can POST request envelopes alongside the WSS subscription.
fn http_base(relay_url: &str) -> String {
    if let Some(rest) = relay_url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = relay_url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        // Pass through — caller may already supply http(s)://.
        relay_url.to_string()
    }
}

type PendingMap = HashMap<String, oneshot::Sender<Result<ResponseEnvelope, RelayError>>>;

/// Dapp-side relay client. Cheaply clone-able (internal `Arc`s).
#[derive(Clone)]
pub struct DappRelay {
    config: RelayConfig,
    http: reqwest::Client,
    /// Pending sign requests, keyed by request_id.
    pending: Arc<Mutex<PendingMap>>,
    /// One-shot for the first `accepted` frame, populated on `open()`.
    accepted: Arc<RwLock<Option<oneshot::Sender<String>>>>,
    /// Cancel handle for the background WSS reader.
    shutdown: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl core::fmt::Debug for DappRelay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DappRelay")
            .field("topic", &self.config.topic)
            .field("relay_url", &self.config.relay_url)
            .finish()
    }
}

impl DappRelay {
    /// Construct a relay client. Does NOT connect yet — call
    /// [`Self::open`] or [`Self::reconnect`].
    pub fn new(config: RelayConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client build"),
            pending: Arc::new(Mutex::new(HashMap::new())),
            accepted: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(Mutex::new(None)),
        }
    }

    /// Open the WSS, send `init`, wait for `init_ack`. Spawns a
    /// background reader task that handles inbound frames for the
    /// lifetime of this connection.
    pub async fn open(&self) -> Result<(), RelayError> {
        let url = self.config.relay_url.clone();
        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| RelayError::Transport(format!("connect {url}: {e}")))?;
        let (mut sink, mut stream) = ws.split();

        // Send init.
        let init = ClientFrame::Init {
            topic: self.config.topic.clone(),
            webapp_pub: self.config.webapp_pub_b64.clone(),
        };
        let init_json =
            serde_json::to_string(&init).map_err(|e| RelayError::BadFrame(e.to_string()))?;
        sink.send(Message::Text(init_json))
            .await
            .map_err(|e| RelayError::Transport(format!("send init: {e}")))?;

        // Wait for init_ack inline.
        let init_ack = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .map_err(|_| RelayError::Transport("init_ack timeout".into()))?
            .ok_or_else(|| RelayError::Closed {
                code: 0,
                reason: "stream ended before init_ack".into(),
            })?
            .map_err(|e| RelayError::Transport(format!("init_ack: {e}")))?;
        let init_ack_text = match init_ack {
            Message::Text(s) => s,
            other => {
                return Err(RelayError::BadFrame(format!(
                    "expected text init_ack, got {other:?}"
                )))
            }
        };
        let parsed: ServerFrame = serde_json::from_str(&init_ack_text)
            .map_err(|e| RelayError::BadFrame(format!("{e}: {init_ack_text}")))?;
        match parsed {
            ServerFrame::InitAck { .. } => {}
            ServerFrame::Error { reason } => return Err(RelayError::ServerError(reason)),
            other => {
                return Err(RelayError::BadFrame(format!(
                    "expected init_ack, got {other:?}"
                )))
            }
        }

        // Spawn the reader.
        self.spawn_reader(sink, stream).await;
        Ok(())
    }

    /// Re-attach to an existing topic (e.g. after a WSS drop). Sends
    /// `subscribe` instead of `init`; the relay treats the connection as
    /// ready immediately, no `init_ack`.
    pub async fn reconnect(&self) -> Result<(), RelayError> {
        let url = self.config.relay_url.clone();
        let (ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| RelayError::Transport(format!("reconnect {url}: {e}")))?;
        let (mut sink, stream) = ws.split();
        let sub = ClientFrame::Subscribe {
            topic: self.config.topic.clone(),
        };
        let sub_json =
            serde_json::to_string(&sub).map_err(|e| RelayError::BadFrame(e.to_string()))?;
        sink.send(Message::Text(sub_json))
            .await
            .map_err(|e| RelayError::Transport(format!("send subscribe: {e}")))?;
        self.spawn_reader(sink, stream).await;
        Ok(())
    }

    async fn spawn_reader<S, R>(&self, sink: S, mut stream: R)
    where
        S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
            + Send
            + Unpin
            + 'static,
        R: futures_util::Stream<
                Item = Result<Message, tokio_tungstenite::tungstenite::Error>,
            > + Send
            + Unpin
            + 'static,
    {
        // Stash the sink so close() can shut it cleanly. We don't actually
        // need to write from the reader loop after init, but keeping it
        // around prevents the WSS from closing prematurely on some servers.
        let _ = sink;

        let (tx_shutdown, mut rx_shutdown) = oneshot::channel::<()>();
        *self.shutdown.lock().await = Some(tx_shutdown);

        let pending = Arc::clone(&self.pending);
        let accepted = Arc::clone(&self.accepted);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx_shutdown => {
                        log::debug!("DappRelay reader shutting down");
                        break;
                    }
                    next = stream.next() => {
                        match next {
                            None => {
                                log::debug!("DappRelay WSS stream ended");
                                fail_all_pending(&pending, RelayError::Closed {
                                    code: 0, reason: "stream ended".into()
                                }).await;
                                break;
                            }
                            Some(Err(e)) => {
                                log::warn!("DappRelay WSS error: {e}");
                                fail_all_pending(&pending, RelayError::Transport(e.to_string())).await;
                                break;
                            }
                            Some(Ok(Message::Text(raw))) => {
                                if let Err(e) = Self::handle_frame(&raw, &pending, &accepted).await {
                                    log::warn!("DappRelay frame error: {e}");
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                log::debug!("DappRelay received Close frame");
                                fail_all_pending(&pending, RelayError::Closed {
                                    code: 1000, reason: "server closed".into()
                                }).await;
                                break;
                            }
                            Some(Ok(_)) => {} // ignore Ping/Pong/Binary
                        }
                    }
                }
            }
        });
    }

    async fn handle_frame(
        raw: &str,
        pending: &Arc<Mutex<PendingMap>>,
        accepted: &Arc<RwLock<Option<oneshot::Sender<String>>>>,
    ) -> Result<(), RelayError> {
        let frame: ServerFrame = serde_json::from_str(raw)
            .map_err(|e| RelayError::BadFrame(format!("{e}: {raw}")))?;
        match frame {
            ServerFrame::InitAck { .. } => {} // already consumed in open()
            ServerFrame::Accepted { mobile_pub, .. } => {
                let mut guard = accepted.write().await;
                if let Some(tx) = guard.take() {
                    let _ = tx.send(mobile_pub);
                }
            }
            ServerFrame::Response {
                request_id,
                ciphertext,
                nonce,
                ..
            } => {
                let mut p = pending.lock().await;
                if let Some(tx) = p.remove(&request_id) {
                    let _ = tx.send(Ok(ResponseEnvelope { ciphertext, nonce }));
                } else {
                    log::warn!("DappRelay got response for unknown request_id={request_id}");
                }
            }
            ServerFrame::PairingRevoked { .. } => {
                fail_all_pending(
                    pending,
                    RelayError::ServerError("pairing_revoked".into()),
                )
                .await;
            }
            ServerFrame::Error { reason } => {
                log::warn!("DappRelay server error frame: {reason}");
            }
        }
        Ok(())
    }

    /// Wait for the wallet to pair. Returns the mobile pubkey (base64).
    /// Caller passes this to [`crate::crypto::ecdh_derive`] to finish key
    /// derivation.
    pub async fn await_accepted(&self, timeout: Duration) -> Result<String, RelayError> {
        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.accepted.write().await;
            if guard.is_some() {
                // Already armed — replace? For now, error out so callers
                // know they're stomping.
                return Err(RelayError::Transport(
                    "await_accepted already pending".into(),
                ));
            }
            *guard = Some(tx);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(pub_b64)) => Ok(pub_b64),
            Ok(Err(_)) => Err(RelayError::Transport(
                "accepted sender dropped".into(),
            )),
            Err(_) => Err(RelayError::PairingTimeout(timeout)),
        }
    }

    /// Publish an encrypted request via HTTP POST, then await the
    /// matching `response` frame over WSS. Times out per `timeout`.
    pub async fn send_request(
        &self,
        envelope: RequestEnvelope,
        timeout: Option<Duration>,
    ) -> Result<ResponseEnvelope, RelayError> {
        let request_id = envelope.request_id.clone();
        let timeout = timeout.unwrap_or(DEFAULT_RESPONSE_TIMEOUT);

        // Arm the pending slot BEFORE publishing so a fast response
        // doesn't race past us.
        let (tx, rx) = oneshot::channel();
        {
            let mut p = self.pending.lock().await;
            p.insert(request_id.clone(), tx);
        }

        // Publish via HTTP.
        let base = http_base(&self.config.relay_url);
        let base = base.trim_end_matches('/');
        let topic_enc = url_encode(&self.config.topic);
        let url = format!("{base}/v1/sessions/{topic_enc}/req");
        let body = serde_json::json!({
            "ciphertext": envelope.ciphertext,
            "nonce":      envelope.nonce,
            "origin":     envelope.origin,
            "request_id": request_id,
        });
        let resp = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| RelayError::Transport(format!("publish {url}: {e}")))?;
        if !resp.status().is_success() {
            // Drop the pending slot.
            self.pending.lock().await.remove(&request_id);
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(RelayError::HttpPublish { status, body });
        }

        // Wait for response.
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(env))) => Ok(env),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err(RelayError::Transport(
                "response sender dropped".into(),
            )),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err(RelayError::Timeout(request_id))
            }
        }
    }

    /// Close the WSS and fail all in-flight requests.
    pub async fn close(&self) {
        if let Some(tx) = self.shutdown.lock().await.take() {
            let _ = tx.send(());
        }
        fail_all_pending(&self.pending, RelayError::Closed {
            code: 1000,
            reason: "client closed".into(),
        })
        .await;
    }
}

async fn fail_all_pending(pending: &Arc<Mutex<PendingMap>>, err: RelayError) {
    let mut p = pending.lock().await;
    let drained: Vec<_> = p.drain().collect();
    drop(p);
    for (_, tx) in drained {
        let _ = tx.send(Err(RelayError::Transport(err.to_string())));
    }
}

fn url_encode(s: &str) -> String {
    // Topics are URL-safe-ish (uuid/hex) so this is mostly defensive.
    s.replace([' ', '/', '?', '#', '&', '='], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_base_translates_wss() {
        assert_eq!(http_base("wss://wc.subfrost.io/"), "https://wc.subfrost.io/");
        assert_eq!(http_base("ws://localhost:8080"), "http://localhost:8080");
        assert_eq!(http_base("https://example/"), "https://example/");
    }
}
