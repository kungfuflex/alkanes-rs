//! Live relay smoke: connect to `wss://wss-tls.subfrost.io/v1/pair`
//! and verify the new WS client establishes a stable WSS connection
//! that doesn't get reset at the tokio-tungstenite frame layer.
//!
//! Marked `#[ignore]` because it requires network reachability to
//! `wss-tls.subfrost.io`. Run with:
//!
//! ```sh
//! cargo test -p tlsfetch-ws --test live_relay -- --ignored
//! ```
//!
//! The bug being hunted manifests as
//! `tungstenite::Error::Protocol("Connection reset without closing
//! handshake")` *at the frame layer* — distinct from a server-
//! initiated protocol-level close (which would surface as
//! `Ok(None)`). The gate is binary: 5s of idle recv with neither
//! error nor close = healthy connection through the relay path.

use std::time::Duration;
use tlsfetch_ws::{WsClient, WsConnectOptions, WsMessage};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires network reachability to wss-tls.subfrost.io"]
async fn live_pair_relay() {
    let url = "wss://wss-tls.subfrost.io/v1/pair";

    let opts = WsConnectOptions {
        connect_timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    };
    let connect = WsClient::connect(url, &opts);
    let mut client = match tokio::time::timeout(
        Duration::from_secs(10),
        connect,
    )
    .await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => panic!("WsClient::connect: {e}"),
        Err(_) => panic!("connect timed out (>10s)"),
    };

    // ARCHITECTURAL-GATE NOTE
    //
    // The connection-reset bug we're hunting fires *at the
    // tokio-tungstenite frame layer*: tungstenite::Error::Protocol(
    // "Connection reset without closing handshake"). It's
    // distinguishable from a server-initiated protocol-level close
    // (which surfaces as Ok(None) — a clean Close frame).
    //
    // To exercise the gate, we just hold the WS open for 5s without
    // sending anything. The bug, when it fires, has been observed
    // to manifest within ~50ms of the dial completing — the server's
    // TCP RST arrives before anything else can.
    //
    // If 5s elapses with no recv() error, the gate passes — the
    // new client established a stable WSS connection through the
    // wss-tls.subfrost.io relay path.
    //
    // (We don't try to drive an actual frtun-pair listen handshake
    // here — that needs a valid bech32m peer name minted on the
    // server side. Tick 2 will exercise the full handshake via the
    // alkanes-rs-develop vendor of this crate.)

    // Hold the connection idle for 5s. The bug manifests as Err —
    // neither None nor Ok(Some(_)) trips the architectural gate.
    let recv = client.recv();
    match tokio::time::timeout(Duration::from_secs(5), recv).await {
        Ok(Ok(Some(WsMessage::Binary(b)))) => {
            eprintln!(
                "live_pair_relay: got binary frame ({} bytes)",
                b.len()
            );
        }
        Ok(Ok(Some(WsMessage::Text(t)))) => {
            eprintln!(
                "live_pair_relay: got text frame ({} bytes)",
                t.len()
            );
        }
        Ok(Ok(Some(WsMessage::Ping(_) | WsMessage::Pong(_)))) => {
            eprintln!("live_pair_relay: got ping/pong (acceptable)");
        }
        Ok(Ok(None)) => {
            // Clean close within 10s — surprising for /v1/pair but
            // not the bug we're hunting.
            eprintln!("live_pair_relay: clean close from server");
        }
        Ok(Err(e)) => {
            panic!(
                "live_pair_relay: WS recv error (architectural gate \
                 FAIL — this is the connection-reset bug we're \
                 hunting): {e}"
            );
        }
        Err(_) => {
            // 5s passed with no frame + no error → the connection
            // is healthy and idle. This is the GOOD path; alkanes-cli
            // hit the reset within ~50ms of send_binary.
            eprintln!(
                "live_pair_relay: 5s idle — \
                 architectural gate PASS"
            );
        }
    }

    // Best-effort close. Ignore the result — at this point the
    // gate has already been decided either way.
    let _ = client.close(1000, "test done").await;
}
