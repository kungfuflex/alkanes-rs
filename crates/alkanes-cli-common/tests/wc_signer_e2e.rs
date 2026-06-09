//! End-to-end signer state-machine test, mirrors the shape of
//! `~/subfrost-mobile/crates/subfrost-wallet-integ-tests/tests/
//! wc_frtun_full_e2e.rs`.
//!
//! We don't dial a real bridge here — `WalletTransport` is a trait, so
//! we plug in a `MockTransport` that:
//!   * spawns a "phone" task in-process that listens on its peer name,
//!     performs the X25519 handshake, decrypts a sign-PSBT envelope,
//!     and ships back a synthetic Result;
//!   * routes `dial`/`listen`/`pair_wake` through tokio mpsc channels.
//!
//! The test asserts the same round-trip property as the canonical
//! subfrost-mobile harness: the request the dapp encrypted is the
//! plaintext the phone decrypted, byte-for-byte.

#![cfg(feature = "wc-signer-native")]

use alkanes_cli_common::wc_signer::{
    crypto,
    signer::WalletConnectSigner,
    storage::{NativeFileStorage, PersistedSession, SessionStorage, StorageError},
    transport::{TransportError, WalletPairStream, WalletTransport},
    wire::{Plaintext, WireEnvelope},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

// ---------------------------------------------------------------------
// Mock transport — channel topology built BEFORE dial returns
// ---------------------------------------------------------------------

/// A logical pair-stream backed by two tokio mpsc channels.
struct ChanStream {
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
    remote: String,
}

#[async_trait]
impl WalletPairStream for ChanStream {
    async fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.tx.send(bytes.to_vec()).await
            .map_err(|e| TransportError::Io(e.to_string()))
    }

    async fn recv_frame(&mut self) -> Result<Vec<u8>, TransportError> {
        self.rx.recv().await.ok_or_else(|| TransportError::Io("channel closed".into()))
    }

    async fn close(&mut self) {}

    fn remote_peer(&self) -> &str {
        &self.remote
    }
}

struct CleanMockTransport {
    /// peer_name → oneshot sender the listener is waiting on. When dial
    /// fires, we mint a paired ChanStream and deliver the phone side
    /// here while returning the dapp side.
    inboxes: Arc<Mutex<HashMap<String, mpsc::Sender<ChanStream>>>>,
    wake_calls: Arc<Mutex<Vec<String>>>,
    listener_up: Arc<Mutex<bool>>,
}

impl CleanMockTransport {
    fn new() -> Self {
        Self {
            inboxes: Arc::new(Mutex::new(HashMap::new())),
            wake_calls: Arc::new(Mutex::new(Vec::new())),
            listener_up: Arc::new(Mutex::new(true)),
        }
    }
}

#[async_trait]
impl WalletTransport for CleanMockTransport {
    async fn dial(
        &self,
        _bridge_url: &str,
        self_peer: &str,
        remote_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError> {
        if !*self.listener_up.lock().await {
            return Err(TransportError::PeerNotFound);
        }
        let inbox = {
            let inboxes = self.inboxes.lock().await;
            match inboxes.get(remote_peer) {
                Some(s) => s.clone(),
                None => return Err(TransportError::PeerNotFound),
            }
        };
        // dapp_tx -> phone_rx; phone_tx -> dapp_rx
        let (dapp_tx, phone_rx) = mpsc::channel::<Vec<u8>>(4);
        let (phone_tx, dapp_rx) = mpsc::channel::<Vec<u8>>(4);
        let dapp_stream = ChanStream {
            tx: dapp_tx,
            rx: dapp_rx,
            remote: remote_peer.to_string(),
        };
        let phone_stream = ChanStream {
            tx: phone_tx,
            rx: phone_rx,
            remote: self_peer.to_string(),
        };
        inbox.send(phone_stream).await
            .map_err(|e| TransportError::Dial(e.to_string()))?;
        Ok(Box::new(dapp_stream))
    }

    async fn listen(
        &self,
        _bridge_url: &str,
        self_peer: &str,
    ) -> Result<Box<dyn WalletPairStream>, TransportError> {
        let (tx, mut rx) = mpsc::channel::<ChanStream>(1);
        {
            let mut inboxes = self.inboxes.lock().await;
            inboxes.insert(self_peer.to_string(), tx);
        }
        let stream = rx.recv().await
            .ok_or_else(|| TransportError::Listen("inbox closed".into()))?;
        Ok(Box::new(stream))
    }

    async fn pair_wake(&self, _bridge_url: &str, peer: &str) -> Result<bool, TransportError> {
        self.wake_calls.lock().await.push(peer.to_string());
        *self.listener_up.lock().await = true;
        Ok(true)
    }
}

// ---------------------------------------------------------------------
// In-memory storage — no filesystem I/O for the test
// ---------------------------------------------------------------------

struct MemStorage {
    inner: Mutex<Option<PersistedSession>>,
}

impl MemStorage {
    fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }
}

#[async_trait]
impl SessionStorage for MemStorage {
    async fn save(&self, session: &PersistedSession) -> Result<(), StorageError> {
        *self.inner.lock().await = Some(session.clone());
        Ok(())
    }
    async fn load(&self) -> Result<Option<PersistedSession>, StorageError> {
        Ok(self.inner.lock().await.clone())
    }
    async fn delete(&self) -> Result<(), StorageError> {
        *self.inner.lock().await = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[tokio::test]
async fn pair_state_machine_mock_full_round_trip() -> anyhow::Result<()> {
    use alkanes_cli_common::wc_signer::pairing;

    let transport = Arc::new(CleanMockTransport::new());
    let storage = Arc::new(MemStorage::new());

    // The phone simulator mints its peer name, but we don't know the
    // pairing_code until the signer calls on_ready. We'll capture the
    // code via a oneshot.
    let (phone_peer, phone_peer_for_sim) = {
        let p = pairing::generate_cli_peer_name(); // shape-compatible
        (p.clone(), p)
    };

    // The phone simulator listens on its peer; the dapp listens on its
    // OWN cli_peer (the signer mints it). After the pair handshake, the
    // dapp opens fresh dials to the PHONE's peer for each request, so
    // the phone needs to listen on phone_peer.
    //
    // But: WalletConnectSigner::pair listens on the dapp's CLI peer and
    // waits for the phone to dial IN. So we need the phone sim to
    // initiate the pair-dial.
    //
    // Order of operations in our mock:
    //   1. signer.pair() spawns; signer listens on cli_peer.
    //   2. Phone task: dial cli_peer → handshake → close.
    //   3. Phone task: listen on phone_peer for subsequent requests.
    //   4. Dapp signer.sign_psbt(): dial phone_peer → req/resp.
    //
    // To wire this we let the phone sim:
    //   a. read the cli_peer + pairing_code from a oneshot (captured
    //      from on_ready)
    //   b. dial cli_peer, do the handshake, derive symKey
    //   c. then listen on its own phone_peer for subsequent requests

    let (init_tx, init_rx) = tokio::sync::oneshot::channel::<(String, String)>();
    let transport_for_phone = transport.clone();
    let phone_peer_clone = phone_peer_for_sim.clone();
    let phone_handle = tokio::spawn(async move {
        let (cli_peer, pairing_code): (String, String) = init_rx.await
            .map_err(|e| anyhow::anyhow!("init oneshot: {e}"))?;

        // 1. Dial the dapp's cli_peer to complete the pair handshake.
        let mut pair_stream = transport_for_phone
            .dial("ws://test/v1/pair", &phone_peer_clone, &cli_peer)
            .await
            .map_err(|e| anyhow::anyhow!("phone dial: {e}"))?;

        let (mobile_priv, mobile_pub) = crypto::gen_keypair();
        let mobile_pub_b64 = crypto::pub_to_b64url(&mobile_pub);
        // Phone sends its pub first (the protocol).
        pair_stream.send_frame(mobile_pub_b64.as_bytes()).await
            .map_err(|e| anyhow::anyhow!("send mobile_pub: {e}"))?;
        // Phone reads dapp's pub.
        let dapp_pub_bytes = pair_stream.recv_frame().await
            .map_err(|e| anyhow::anyhow!("recv dapp_pub: {e}"))?;
        let dapp_pub_b64 = std::str::from_utf8(&dapp_pub_bytes)
            .map_err(|e| anyhow::anyhow!("dapp_pub utf8: {e}"))?
            .trim();
        let dapp_pub = crypto::pub_from_b64url(dapp_pub_b64)
            .map_err(|e| anyhow::anyhow!("dapp_pub: {e}"))?;
        let info = format!("{}:{}", phone_peer_clone, pairing_code);
        let sym_key = crypto::ecdh_derive(&mobile_priv, &dapp_pub, &info)
            .map_err(|e| anyhow::anyhow!("phone ecdh: {e}"))?;
        pair_stream.close().await;
        drop(pair_stream);

        // 2. Listen on phone_peer for subsequent encrypted requests.
        for _ in 0..3u32 {
            let mut req_stream = transport_for_phone
                .listen("ws://test/v1/pair", &phone_peer_clone)
                .await
                .map_err(|e| anyhow::anyhow!("phone listen: {e}"))?;
            let env_bytes = req_stream.recv_frame().await
                .map_err(|e| anyhow::anyhow!("recv env: {e}"))?;
            let env: WireEnvelope = serde_json::from_slice(&env_bytes)?;
            let pt = crypto::decrypt_envelope(&sym_key, &env)
                .map_err(|e| anyhow::anyhow!("decrypt: {e}"))?;
            let plain: Plaintext = serde_json::from_slice(&pt)?;
            let resp = match plain {
                Plaintext::SignPsbt { request_id, psbt_hex, .. } => Plaintext::Result {
                    request_id,
                    result: psbt_hex,
                },
                Plaintext::SignMessage { request_id, message, .. } => Plaintext::Result {
                    request_id,
                    result: format!("sig({message})"),
                },
                Plaintext::GetAccounts { request_id, .. } => Plaintext::Accounts {
                    request_id,
                    addresses: vec!["bc1qaccount1".into(), "bc1qaccount2".into()],
                },
                other => Plaintext::Error {
                    request_id: "x".into(),
                    code: "internal".into(),
                    message: format!("unexpected: {other:?}"),
                },
            };
            let resp_json = serde_json::to_vec(&resp)?;
            let resp_env = crypto::encrypt_to_envelope(&sym_key, &resp_json)?;
            let resp_env_bytes = serde_json::to_vec(&resp_env)?;
            req_stream.send_frame(&resp_env_bytes).await?;
            req_stream.close().await;
        }
        Ok::<(), anyhow::Error>(())
    });

    // Drive the signer pair flow in this task. We capture the cli_peer
    // + pairing_code from the on_ready hook into the oneshot so the
    // phone task can wake up and dial.
    let init_tx_cell = Arc::new(Mutex::new(Some(init_tx)));
    let init_tx_for_cb = init_tx_cell.clone();

    let mut signer = WalletConnectSigner::pair(
        transport.clone(),
        storage.clone(),
        "cli://test".to_string(),
        "ws://test/v1/pair".to_string(),
        move |init| {
            // Snap cli_peer + pairing_code through to the phone sim.
            let tx = init_tx_for_cb.try_lock().ok().and_then(|mut g| g.take());
            if let Some(tx) = tx {
                let _ = tx.send((init.cli_peer_name.clone(), init.pairing_code.clone()));
            }
        },
        Duration::from_secs(5),
    ).await?;

    // signer is paired. Now exercise each request shape.
    let accounts = signer.get_accounts().await?;
    assert_eq!(accounts, vec!["bc1qaccount1", "bc1qaccount2"],
        "phone sim's account list should round-trip");

    let signed = signer.sign_psbt(
        "deadbeef".to_string(),
        vec!["bc1qaccount1".to_string()],
    ).await?;
    assert_eq!(signed, "deadbeef",
        "phone sim echoes psbt_hex as synthetic signed result");

    let sig = signer.sign_message(
        "hello".to_string(),
        "bc1qaccount1".to_string(),
    ).await?;
    assert_eq!(sig, "sig(hello)",
        "phone sim wraps message in sig(...) as synthetic signature");

    // Drive the phone sim to completion (3 requests).
    let _ = tokio::time::timeout(Duration::from_secs(3), phone_handle).await;

    // Persistence sanity: a fresh restore from the same storage should
    // yield a signer that knows the cached accounts.
    let restored = WalletConnectSigner::restore(transport.clone(), storage.clone()).await?;
    assert_eq!(restored.accounts(), vec!["bc1qaccount1", "bc1qaccount2"]);

    Ok(())
}

#[tokio::test]
async fn wake_then_retry_succeeds_after_peer_not_found() -> anyhow::Result<()> {
    // This test pre-seeds a paired session in storage, then simulates
    // a dial that hits PeerNotFound; expect WalletConnectSigner to call
    // pair_wake, flip the listener_up bit, and retry successfully.
    let transport = Arc::new(CleanMockTransport::new());
    let storage = Arc::new(MemStorage::new());

    // Pre-seed: phone's peer is "phone-x", we cooked a symKey ahead of
    // time, etc. We'll skip the pair handshake entirely.
    let phone_peer = "frtun1deadbeef.peer".to_string();
    let cli_peer = "frtun1abcdef00.peer".to_string();
    let (own_priv, own_pub) = crypto::gen_keypair();
    let (peer_priv, peer_pub) = crypto::gen_keypair();
    let pairing_code = "ZZZZZZ".to_string();
    let info = format!("{}:{}", phone_peer, pairing_code);
    let sym_key = crypto::ecdh_derive(&own_priv, &peer_pub, &info).unwrap();
    let session = PersistedSession {
        cli_peer_name: cli_peer.clone(),
        wallet_peer_name: phone_peer.clone(),
        bridge_url: "ws://test/v1/pair".to_string(),
        origin: "cli://test".to_string(),
        pairing_code: pairing_code.clone(),
        sym_key_b64: crypto::b64url_encode(&sym_key),
        own_priv_b64: crypto::b64url_encode(own_priv.as_bytes()),
        own_pub_b64: crypto::pub_to_b64url(&own_pub),
        peer_pub_b64: crypto::pub_to_b64url(&peer_pub),
        accounts: vec![],
        paired_at: "x".into(),
        last_used_at: "x".into(),
    };
    storage.save(&session).await.unwrap();

    // Listener starts DOWN — first dial will return PeerNotFound.
    *transport.listener_up.lock().await = false;

    // Spawn a phone sim that will register a listener AFTER the wake
    // call flips listener_up. We arm it by polling the wake_calls vec.
    let transport_for_phone = transport.clone();
    let phone_peer_clone = phone_peer.clone();
    let peer_priv_clone = peer_priv;
    let info_clone = info.clone();
    let phone_handle = tokio::spawn(async move {
        // Wait for wake to fire.
        loop {
            if !transport_for_phone.wake_calls.lock().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Now register listener.
        let mut req_stream = transport_for_phone
            .listen("ws://test/v1/pair", &phone_peer_clone)
            .await
            .map_err(|e| anyhow::anyhow!("phone listen after wake: {e}"))?;
        let env_bytes = req_stream.recv_frame().await
            .map_err(|e| anyhow::anyhow!("recv env: {e}"))?;
        let env: WireEnvelope = serde_json::from_slice(&env_bytes)?;
        let pt = crypto::decrypt_envelope(&derive_phone_sym(&peer_priv_clone, &session.own_pub_b64, &info_clone)?, &env)
            .map_err(|e| anyhow::anyhow!("decrypt: {e}"))?;
        let plain: Plaintext = serde_json::from_slice(&pt)?;
        let resp = match plain {
            Plaintext::GetAccounts { request_id, .. } => Plaintext::Accounts {
                request_id,
                addresses: vec!["bc1qpostwake".into()],
            },
            other => Plaintext::Error {
                request_id: "x".into(),
                code: "internal".into(),
                message: format!("{other:?}"),
            },
        };
        let resp_json = serde_json::to_vec(&resp)?;
        let resp_env = crypto::encrypt_to_envelope(&derive_phone_sym(&peer_priv_clone, &session.own_pub_b64, &info_clone)?, &resp_json)?;
        let resp_env_bytes = serde_json::to_vec(&resp_env)?;
        req_stream.send_frame(&resp_env_bytes).await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut signer = WalletConnectSigner::restore(transport.clone(), storage.clone()).await?;
    let accounts = tokio::time::timeout(Duration::from_secs(10), signer.get_accounts()).await??;
    assert_eq!(accounts, vec!["bc1qpostwake"], "wake → retry → success");

    // Wake fired exactly once.
    assert_eq!(transport.wake_calls.lock().await.len(), 1);

    let _ = phone_handle.await;
    Ok(())
}

fn derive_phone_sym(
    peer_priv: &x25519_dalek::StaticSecret,
    own_pub_b64: &str,
    info: &str,
) -> anyhow::Result<[u8; 32]> {
    let dapp_pub = crypto::pub_from_b64url(own_pub_b64)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    crypto::ecdh_derive(peer_priv, &dapp_pub, info)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

#[tokio::test]
async fn wallet_error_propagates_as_wallet_rejected() -> anyhow::Result<()> {
    // Pre-seed a session, return a Plaintext::Error from the phone sim,
    // assert we get WcError::WalletRejected with the expected code.
    let transport = Arc::new(CleanMockTransport::new());
    let storage = Arc::new(MemStorage::new());

    let phone_peer = "frtun1deadbeef.peer".to_string();
    let cli_peer = "frtun1abcdef01.peer".to_string();
    let (own_priv, own_pub) = crypto::gen_keypair();
    let (peer_priv, peer_pub) = crypto::gen_keypair();
    let pairing_code = "QQQQQQ".to_string();
    let info = format!("{}:{}", phone_peer, pairing_code);
    let sym_key = crypto::ecdh_derive(&own_priv, &peer_pub, &info).unwrap();
    let session = PersistedSession {
        cli_peer_name: cli_peer.clone(),
        wallet_peer_name: phone_peer.clone(),
        bridge_url: "ws://test/v1/pair".to_string(),
        origin: "cli://test".to_string(),
        pairing_code: pairing_code.clone(),
        sym_key_b64: crypto::b64url_encode(&sym_key),
        own_priv_b64: crypto::b64url_encode(own_priv.as_bytes()),
        own_pub_b64: crypto::pub_to_b64url(&own_pub),
        peer_pub_b64: crypto::pub_to_b64url(&peer_pub),
        accounts: vec![],
        paired_at: "x".into(),
        last_used_at: "x".into(),
    };
    storage.save(&session).await.unwrap();

    let transport_for_phone = transport.clone();
    let phone_peer_clone = phone_peer.clone();
    let info_clone = info.clone();
    let session_pub = session.own_pub_b64.clone();
    let phone_handle = tokio::spawn(async move {
        let mut req_stream = transport_for_phone
            .listen("ws://test/v1/pair", &phone_peer_clone)
            .await
            .map_err(|e| anyhow::anyhow!("listen: {e}"))?;
        let env_bytes = req_stream.recv_frame().await
            .map_err(|e| anyhow::anyhow!("recv: {e}"))?;
        let env: WireEnvelope = serde_json::from_slice(&env_bytes)?;
        let sym = derive_phone_sym(&peer_priv, &session_pub, &info_clone)?;
        let pt = crypto::decrypt_envelope(&sym, &env)
            .map_err(|e| anyhow::anyhow!("decrypt: {e}"))?;
        let plain: Plaintext = serde_json::from_slice(&pt)?;
        let request_id = match plain {
            Plaintext::SignPsbt { request_id, .. } => request_id,
            _ => "x".into(),
        };
        let resp = Plaintext::Error {
            request_id,
            code: "user_rejected".into(),
            message: "user tapped Reject".into(),
        };
        let resp_json = serde_json::to_vec(&resp)?;
        let resp_env = crypto::encrypt_to_envelope(&sym, &resp_json)?;
        let resp_env_bytes = serde_json::to_vec(&resp_env)?;
        req_stream.send_frame(&resp_env_bytes).await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut signer = WalletConnectSigner::restore(transport.clone(), storage.clone()).await?;
    let err = signer.sign_psbt("deadbeef".to_string(), vec![]).await.unwrap_err();
    match err {
        alkanes_cli_common::wc_signer::WcError::WalletRejected { code, message } => {
            assert_eq!(code, "user_rejected");
            assert!(message.contains("Reject"));
        }
        other => panic!("expected WalletRejected, got {other:?}"),
    }

    let _ = phone_handle.await;
    Ok(())
}

#[tokio::test]
async fn storage_native_file_round_trip() -> anyhow::Result<()> {
    // Exercise the file-backed storage shipped as the default native
    // impl. Use a tempfile so we don't pollute ~/.alkanes.
    let tmp = tempfile::NamedTempFile::new()?;
    let path = tmp.path().to_path_buf();
    drop(tmp);
    let storage = NativeFileStorage::new(path);

    let s = PersistedSession {
        cli_peer_name: "frtun1a.peer".into(),
        wallet_peer_name: "frtun1b.peer".into(),
        bridge_url: "wss://wss-tls.subfrost.io/v1/pair".into(),
        origin: "cli://x".into(),
        pairing_code: "ABCDEF".into(),
        sym_key_b64: "AAAA".into(),
        own_priv_b64: "BBBB".into(),
        own_pub_b64: "CCCC".into(),
        peer_pub_b64: "DDDD".into(),
        accounts: vec!["bc1qa".into()],
        paired_at: "now".into(),
        last_used_at: "now".into(),
    };
    storage.save(&s).await?;
    let loaded = storage.load().await?.expect("session");
    assert_eq!(loaded.cli_peer_name, s.cli_peer_name);
    assert_eq!(loaded.accounts, s.accounts);
    storage.delete().await?;
    let after = storage.load().await?;
    assert!(after.is_none());
    Ok(())
}
