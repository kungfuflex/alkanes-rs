//! Loopback test: stand up a tokio-tungstenite echo server on
//! 127.0.0.1:0, dial it with our `WsClient`, round-trip one binary
//! frame, close cleanly.

use std::time::Duration;
use tlsfetch_ws::{WsClient, WsConnectOptions, WsMessage};
use tokio::net::TcpListener;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binary_round_trip() {
    let _ = tokio::time::timeout(Duration::from_secs(5), inner())
        .await
        .expect("loopback test timed out");
}

async fn inner() {
    // Bind an ephemeral port and capture it for the dial URL.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let port = addr.port();

    // Server task: accept ONE connection, echo every binary frame
    // back. Exits when the client closes.
    let server = tokio::spawn(async move {
        let (sock, _peer) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(sock)
            .await
            .expect("accept_async");
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        while let Some(msg) = ws.next().await {
            match msg {
                Ok(Message::Binary(b)) => {
                    ws.send(Message::Binary(b)).await.expect("server send");
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    });

    let url = format!("ws://127.0.0.1:{port}/");
    let mut client = WsClient::connect(&url, &WsConnectOptions::default())
        .await
        .expect("connect");

    let payload = bytes::Bytes::from_static(b"\x00\x01\x02hello-tlsfetch-ws");
    client.send_binary(payload.clone()).await.expect("send");

    let got = client.recv().await.expect("recv ok");
    match got {
        Some(WsMessage::Binary(b)) => {
            assert_eq!(b.as_ref(), payload.as_ref(), "echo mismatch");
        }
        other => panic!("expected Binary, got {other:?}"),
    }

    client.close(1000, "bye").await.expect("close");
    // Let the server task drain. Ignore its result either way.
    let _ = tokio::time::timeout(Duration::from_secs(1), server).await;
}
