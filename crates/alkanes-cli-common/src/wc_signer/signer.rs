//! `WalletConnectSigner` — the cross-platform, transport-+-storage-
//! generic signer driver. Mirrors the TS dapp state machine in
//! `~/subfrost-mobile/ts-sdk/src/cli.ts::cmdPair` + `sendOverFrtun`.
//!
//! Flow:
//!   1. `pair()` — mint identity + URI + listen on bridge; phone dials;
//!      exchange X25519 pubs; ECDH-derive symKey with `phone_peer:code`
//!      HKDF info; persist session; return signer ready to sign.
//!   2. `restore()` — load previously-paired session from storage.
//!   3. `get_accounts()` / `sign_psbt()` / `sign_message()` — each opens
//!      a fresh `dial` to the phone's peer (with wake-on-peer_not_found
//!      retry), ships one encrypted envelope, reads one response frame,
//!      closes the stream.

use crate::wc_signer::{
    crypto::{self, KEY_LEN},
    pairing,
    storage::{PersistedSession, SessionStorage},
    transport::{WalletPairStream, WalletTransport},
    wire::{Plaintext, WireEnvelope},
};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Debug, Error)]
pub enum WcError {
    #[error("not paired (no session)")]
    NotPaired,
    #[error("crypto: {0}")]
    Crypto(#[from] crypto::CryptoError),
    #[error("transport: {0}")]
    Transport(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("wallet rejected: [{code}] {message}")]
    WalletRejected { code: String, message: String },
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("timeout after {0:?}")]
    Timeout(Duration),
}

impl From<crate::wc_signer::transport::TransportError> for WcError {
    fn from(e: crate::wc_signer::transport::TransportError) -> Self {
        WcError::Transport(e.to_string())
    }
}

impl From<crate::wc_signer::storage::StorageError> for WcError {
    fn from(e: crate::wc_signer::storage::StorageError) -> Self {
        WcError::Storage(e.to_string())
    }
}

/// Per-request UUIDv4-shaped string. We don't pull in the `uuid` crate
/// here (the canonical sender ships request_ids as opaque strings) — a
/// 32-hex blob is byte-identical-good-enough for the receiver.
fn random_request_id() -> String {
    use rand::{rngs::OsRng, RngCore};
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    let mut s = String::with_capacity(36);
    const HEX: &[u8] = b"0123456789abcdef";
    for (i, byte) in buf.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        s.push(HEX[(byte >> 4) as usize] as char);
        s.push(HEX[(byte & 0x0f) as usize] as char);
    }
    s
}

/// The high-level signer driver. Holds the persisted session + transport
/// + storage handles. Cheap to clone — all the heavy fields are `Arc`.
pub struct WalletConnectSigner<T: WalletTransport + 'static, S: SessionStorage + 'static> {
    transport: Arc<T>,
    storage: Arc<S>,
    session: PersistedSession,
    sym_key: [u8; KEY_LEN],
}

impl<T: WalletTransport + 'static, S: SessionStorage + 'static> WalletConnectSigner<T, S> {
    /// Mint identity + URI + listen on the bridge; block until the phone
    /// dials, swaps X25519 pubs, derives symKey. `on_ready` fires once
    /// with the deeplink URI + pairing code so callers can render it.
    pub async fn pair<F>(
        transport: Arc<T>,
        storage: Arc<S>,
        origin: String,
        bridge_url: String,
        on_ready: F,
        timeout: Duration,
    ) -> Result<Self, WcError>
    where
        F: FnOnce(&PairInit),
    {
        // 1. Mint identity.
        let (own_priv, own_pub) = crypto::gen_keypair();
        let own_pub_b64 = crypto::pub_to_b64url(&own_pub);
        let cli_peer = pairing::generate_cli_peer_name();
        let pairing_code = pairing::generate_pairing_code();

        // 2. Build deeplink + surface to caller.
        let uri = pairing::build_pair_uri(
            &cli_peer,
            &own_pub_b64,
            &pairing_code,
            &bridge_url,
            &origin,
            "cli",
        );
        let init = PairInit {
            deeplink: uri,
            pairing_code: pairing_code.clone(),
            cli_peer_name: cli_peer.clone(),
            bridge_url: bridge_url.clone(),
        };
        on_ready(&init);

        // 3. Listen on the bridge under our peer name; phone will dial.
        let listen_fut = transport.listen(&bridge_url, &cli_peer);
        let mut stream = tokio_timeout(timeout, listen_fut).await??;

        let wallet_peer = stream.remote_peer().to_string();

        // 4. Read phone's mobile_pub (first binary frame, b64url-43).
        let mobile_pub_bytes = stream.recv_frame().await?;
        let mobile_pub_b64 = std::str::from_utf8(&mobile_pub_bytes)
            .map_err(|e| WcError::Protocol(format!("mobile_pub utf8: {e}")))?
            .trim();
        let mobile_pub: PublicKey = crypto::pub_from_b64url(mobile_pub_b64)?;
        let peer_pub_b64 = mobile_pub_b64.to_string();

        // 5. Send our pub in the second binary frame.
        stream.send_frame(own_pub_b64.as_bytes()).await?;

        // 6. ECDH-derive symKey. info = "<wallet_peer>:<pairing_code>".
        let info = format!("{}:{}", wallet_peer, pairing_code);
        let sym_key = crypto::ecdh_derive(&own_priv, &mobile_pub, &info)?;

        // 7. Close the pair stream; subsequent requests open fresh dials.
        stream.close().await;

        // 8. Persist.
        let now = iso_now();
        let session = PersistedSession {
            cli_peer_name: cli_peer,
            wallet_peer_name: wallet_peer,
            bridge_url,
            origin,
            pairing_code,
            sym_key_b64: crypto::b64url_encode(&sym_key),
            own_priv_b64: crypto::b64url_encode(own_priv.as_bytes()),
            own_pub_b64,
            peer_pub_b64,
            accounts: Vec::new(),
            paired_at: now.clone(),
            last_used_at: now,
        };
        storage.save(&session).await?;

        Ok(Self {
            transport,
            storage,
            session,
            sym_key,
        })
    }

    /// Restore a previously-paired session from storage.
    pub async fn restore(transport: Arc<T>, storage: Arc<S>) -> Result<Self, WcError> {
        let session = storage.load().await?.ok_or(WcError::NotPaired)?;
        let sym_bytes = crypto::b64url_decode(&session.sym_key_b64)?;
        if sym_bytes.len() != KEY_LEN {
            return Err(WcError::Protocol(format!(
                "stored sym_key length {} != {}", sym_bytes.len(), KEY_LEN
            )));
        }
        let mut sym_key = [0u8; KEY_LEN];
        sym_key.copy_from_slice(&sym_bytes);
        Ok(Self {
            transport,
            storage,
            session,
            sym_key,
        })
    }

    /// Cached accounts, if any. Use `get_accounts` to force-fetch.
    pub fn accounts(&self) -> Vec<String> {
        self.session.accounts.clone()
    }

    pub fn session(&self) -> &PersistedSession {
        &self.session
    }

    /// Round-trip a `Plaintext::GetAccounts` request to the phone.
    pub async fn get_accounts(&mut self) -> Result<Vec<String>, WcError> {
        let request_id = random_request_id();
        let req = Plaintext::GetAccounts {
            request_id: request_id.clone(),
            origin: self.session.origin.clone(),
        };
        let resp = self.send_request(&req).await?;
        match resp {
            Plaintext::Accounts { request_id: rid, addresses } => {
                if rid != request_id {
                    return Err(WcError::Protocol(format!(
                        "response request_id mismatch: {rid} vs {request_id}"
                    )));
                }
                self.session.accounts = addresses.clone();
                self.session.last_used_at = iso_now();
                self.storage.save(&self.session).await?;
                Ok(addresses)
            }
            Plaintext::Error { code, message, .. } => {
                Err(WcError::WalletRejected { code, message })
            }
            other => Err(WcError::Protocol(format!("expected Accounts, got {other:?}"))),
        }
    }

    /// Ship a `Plaintext::SignPsbt` request + return the signed hex.
    pub async fn sign_psbt(
        &mut self,
        psbt_hex: String,
        addresses: Vec<String>,
    ) -> Result<String, WcError> {
        let request_id = random_request_id();
        let req = Plaintext::SignPsbt {
            psbt_hex,
            addresses,
            request_id: request_id.clone(),
            origin: self.session.origin.clone(),
        };
        let resp = self.send_request(&req).await?;
        self.expect_result(resp, &request_id).await
    }

    /// Ship a `Plaintext::SignMessage` request + return the base64
    /// signature (BIP-137 for non-taproot, BIP-340 for taproot, as the
    /// SUBFROST mobile WC handler shipped at vc=171).
    pub async fn sign_message(
        &mut self,
        message: String,
        address: String,
    ) -> Result<String, WcError> {
        let request_id = random_request_id();
        let req = Plaintext::SignMessage {
            message,
            address,
            request_id: request_id.clone(),
            origin: self.session.origin.clone(),
        };
        let resp = self.send_request(&req).await?;
        self.expect_result(resp, &request_id).await
    }

    /// Send + read on one fresh stream, with the wake-and-retry dance
    /// when the phone's listener isn't up yet.
    async fn send_request(&self, req: &Plaintext) -> Result<Plaintext, WcError> {
        // 1. Encrypt.
        let req_json = serde_json::to_vec(req)
            .map_err(|e| WcError::Protocol(format!("serialize req: {e}")))?;
        let envelope = crypto::encrypt_to_envelope(&self.sym_key, &req_json)?;
        let env_bytes = serde_json::to_vec(&envelope)
            .map_err(|e| WcError::Protocol(format!("serialize envelope: {e}")))?;

        // 2. Dial with wake-and-retry.
        let mut stream = self.dial_with_wake().await?;

        // 3. One binary frame out, one frame in.
        stream.send_frame(&env_bytes).await?;
        let resp_bytes = stream.recv_frame().await?;
        stream.close().await;

        // 4. Decrypt + parse.
        let resp_env: WireEnvelope = serde_json::from_slice(&resp_bytes)
            .map_err(|e| WcError::Protocol(format!("parse resp env: {e}")))?;
        let pt = crypto::decrypt_envelope(&self.sym_key, &resp_env)?;
        let plaintext: Plaintext = serde_json::from_slice(&pt)
            .map_err(|e| WcError::Protocol(format!("parse resp plaintext: {e}")))?;
        Ok(plaintext)
    }

    async fn expect_result(
        &mut self,
        resp: Plaintext,
        request_id: &str,
    ) -> Result<String, WcError> {
        match resp {
            Plaintext::Result { request_id: rid, result } => {
                if rid != request_id {
                    return Err(WcError::Protocol(format!(
                        "response request_id mismatch: {rid} vs {request_id}"
                    )));
                }
                self.session.last_used_at = iso_now();
                let _ = self.storage.save(&self.session).await;
                Ok(result)
            }
            Plaintext::Error { code, message, .. } => {
                Err(WcError::WalletRejected { code, message })
            }
            other => Err(WcError::Protocol(format!("expected Result, got {other:?}"))),
        }
    }

    /// First attempt: maybe the phone's listener is already up. On
    /// `peer_not_found`, POST `/v1/pair-wake` then retry with a
    /// backoff up to 30s. Mirrors the TS `dialWalletWithWake` helper.
    async fn dial_with_wake(&self) -> Result<Box<dyn WalletPairStream>, WcError> {
        let bridge = &self.session.bridge_url;
        let cli = &self.session.cli_peer_name;
        let wallet = &self.session.wallet_peer_name;

        // Attempt 1.
        match self.transport.dial(bridge, cli, wallet).await {
            Ok(s) => return Ok(s),
            Err(e) if e.is_peer_not_found() => {
                // Fire wake and retry.
            }
            Err(e) => return Err(WcError::Transport(e.to_string())),
        }

        // Wake.
        let _ = self.transport.pair_wake(bridge, wallet).await; // best-effort

        // Retry with backoff up to 30s.
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let backoff_ms = std::cmp::min(500 + attempt * 250, 2500) as u64;
            sleep_compat(Duration::from_millis(backoff_ms)).await;

            match self.transport.dial(bridge, cli, wallet).await {
                Ok(s) => return Ok(s),
                Err(e) if e.is_peer_not_found() => {
                    if std::time::Instant::now() >= deadline {
                        return Err(WcError::Timeout(Duration::from_secs(30)));
                    }
                    continue;
                }
                Err(e) => return Err(WcError::Transport(e.to_string())),
            }
        }
    }
}

/// Returned by `pair()` for the caller to render the deeplink + code.
#[derive(Debug, Clone)]
pub struct PairInit {
    pub deeplink: String,
    pub pairing_code: String,
    pub cli_peer_name: String,
    pub bridge_url: String,
}

// ---------- small platform-spread helpers ----------

fn iso_now() -> String {
    // `chrono` is already in the workspace.
    chrono::Utc::now().to_rfc3339()
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep_compat(d: Duration) {
    tokio::time::sleep(d).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep_compat(d: Duration) {
    let _ = gloo_timers::future::TimeoutFuture::new(d.as_millis() as u32).await;
}

#[cfg(not(target_arch = "wasm32"))]
async fn tokio_timeout<F: std::future::Future>(
    d: Duration,
    f: F,
) -> Result<F::Output, WcError> {
    tokio::time::timeout(d, f).await.map_err(|_| WcError::Timeout(d))
}

#[cfg(target_arch = "wasm32")]
async fn tokio_timeout<F: std::future::Future>(
    _d: Duration,
    f: F,
) -> Result<F::Output, WcError> {
    Ok(f.await)
}
