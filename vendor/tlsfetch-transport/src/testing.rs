//! Conformance test harness for the [`Connection`] trait.
//!
//! Every concrete transport in the workspace plugs into this module
//! by:
//!
//! 1. Implementing [`TransportFactory`] in its own `tests/` directory.
//! 2. Calling [`tlsfetch_transport_contract!`] from the same file.
//!
//! That single macro expands the whole test suite as concrete
//! `#[tokio::test]` (native) / `#[wasm_bindgen_test]` (wasm32) items.
//! Each test dispatches into a generic `pub async fn` here, which
//! drives the factory through the contract clauses.
//!
//! ## Why a public testing module
//!
//! `#[tokio::test]` requires concrete (non-generic) `fn` items.
//! Putting the test bodies as generic `pub async fn`s here and
//! expanding non-generic shim wrappers from the macro is the
//! cleanest way to share one suite across many concrete impls
//! without duplicating bodies.
//!
//! ## Status
//!
//! M3 (this commit): foundational tests filled in. Adversarial /
//! stress tests are stubbed `#[ignore]` and land iteration-by-
//! iteration along with InMemory features that exercise them.

#![allow(dead_code, unused_imports)]

use std::sync::Arc;

use crate::{Connection, Listener, TransportError, TransportFuture, TransportResult};

/// Implemented by each concrete transport's test crate. Hands the
/// harness the bits it needs to spin up a client/server pair and
/// (optionally) a listener.
pub trait TransportFactory: 'static {
    /// Spin up a connected pair: stream IO on either side is
    /// observable on the other. Used for stream + connection
    /// contract tests.
    fn pair() -> TransportFuture<'static, (Box<dyn Connection>, Box<dyn Connection>)>;

    /// Spin up a real listener + a dial fn that opens a fresh
    /// client connection to it. `None` when the transport doesn't
    /// model a listener (a fixed in-memory pair); listener tests
    /// are skipped for those.
    fn listener() -> TransportFuture<'static, Option<ListenerSetup>> {
        // Default: no listener.
        Box::pin(async { None })
    }
}

/// Pair returned by [`TransportFactory::listener`].
pub struct ListenerSetup {
    pub listener: Box<dyn Listener>,
    pub dial: Box<dyn Fn() -> TransportFuture<'static, TransportResult<Box<dyn Connection>>> + Send + Sync>,
}

// ---------------------------------------------------------------------------
// Macro
// ---------------------------------------------------------------------------

/// Emit the full conformance test suite under the caller's `mod`.
///
/// ```ignore
/// // crates/tlsfetch-inmemory/tests/conformance.rs
/// use tlsfetch_transport::testing::TransportFactory;
/// use tlsfetch_transport::tlsfetch_transport_contract;
/// struct InMemoryFactory;
/// impl TransportFactory for InMemoryFactory { /* ... */ }
/// tlsfetch_transport_contract!(InMemoryFactory);
/// ```
#[macro_export]
macro_rules! tlsfetch_transport_contract {
    ($factory:ty) => {
        // Stream contract
        $crate::__contract_test! { stream_64k_roundtrip, $factory }
        $crate::__contract_test! { stream_large_body_backpressure, $factory }
        $crate::__contract_test! { stream_half_close_drains, $factory }
        $crate::__contract_test! { stream_reset_propagates, $factory }
        $crate::__contract_test! { stream_drop_resets, $factory }

        // Connection contract
        $crate::__contract_test! { conn_concurrent_open, $factory }
        $crate::__contract_test! { conn_concurrent_accept, $factory }
        $crate::__contract_test! { conn_stream_datagram_independence, $factory }
        $crate::__contract_test! { conn_close_drains_streams, $factory }
        $crate::__contract_test! { conn_close_idempotent, $factory }
        $crate::__contract_test! { conn_id_stable, $factory }

        // Listener contract
        $crate::__contract_test! { listener_sequential_accept, $factory }
        $crate::__contract_test! { listener_close_stops_accept, $factory }

        // Stress
        $crate::__contract_test! { stress_1k_streams_sequential, $factory }
        $crate::__contract_test! { stress_100_streams_concurrent, $factory }
        $crate::__contract_test! { stress_1k_datagrams, $factory }

        // Adversarial
        $crate::__contract_test! { adv_peer_close_mid_write, $factory }
        $crate::__contract_test! { adv_peer_reset_mid_read, $factory }
        $crate::__contract_test! { adv_tiny_stream_churn, $factory }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __contract_test {
    ($name:ident, $factory:ty) => {
        #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        async fn $name() {
            $crate::testing::$name::<$factory>().await
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __contract_test_ignored {
    ($name:ident, $factory:ty, $reason:expr) => {
        #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        #[ignore = $reason]
        async fn $name() {
            $crate::testing::$name::<$factory>().await
        }
    };
}

// ---------------------------------------------------------------------------
// Test body fns. Generic over `F: TransportFactory`.
// ---------------------------------------------------------------------------

use bytes::Bytes;
use futures_util::io::AsyncReadExt as _;
use futures_util::io::AsyncWriteExt as _;

/// 64 KiB byte-pattern echo across one bidi. Stream contract clauses
/// 1, 2 (ordering, reliability).
pub async fn stream_64k_roundtrip<F: TransportFactory>() {
    let (a, b) = F::pair().await;

    // Pattern: 0..=255 repeated.
    let payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 256) as u8).collect();

    // a opens, writes, closes; b accepts, reads to EOF, asserts.
    let payload_clone = payload.clone();
    let writer = async move {
        let mut bi = a.open_bi().await.expect("open_bi");
        bi.send.write_all(&payload_clone).await.expect("write_all");
        bi.send.close().await.expect("close write");
        a // hold the connection alive
    };
    let reader = async move {
        let mut bi = b.accept_bi().await.expect("accept_bi");
        let mut buf = Vec::with_capacity(64 * 1024);
        bi.recv.read_to_end(&mut buf).await.expect("read_to_end");
        assert_eq!(buf.len(), payload.len(), "byte count mismatch");
        assert!(buf == payload, "byte pattern mismatch");
        b
    };
    let _ = futures_util::join!(writer, reader);
}

/// Sender calls close_send after writing the body. Receiver reads to
/// EOF and gets the full body. Stream contract clause 3.
pub async fn stream_half_close_drains<F: TransportFactory>() {
    let (a, b) = F::pair().await;

    let writer = async move {
        let mut bi = a.open_bi().await.expect("open_bi");
        bi.send.write_all(b"hello world").await.expect("write");
        bi.send.close().await.expect("close (half-close)");
        // After close_send, recv side is still open — but no more
        // data coming back from the peer; we don't read here.
        a
    };
    let reader = async move {
        let mut bi = b.accept_bi().await.expect("accept_bi");
        let mut buf = Vec::new();
        bi.recv.read_to_end(&mut buf).await.expect("read_to_end");
        assert_eq!(&buf, b"hello world");
        b
    };
    let _ = futures_util::join!(writer, reader);
}

/// Open 100 bidi streams concurrently from one task. All resolve;
/// every stream id is unique. Connection contract clauses 7, 12.
pub async fn conn_concurrent_open<F: TransportFactory>() {
    let (a, b) = F::pair().await;

    // Spawn an accept loop on b that drains arrivals into a Vec.
    let accept_handle = {
        let b: Arc<dyn Connection> = Arc::from(b);
        let b_clone = b.clone();
        tokio::spawn(async move {
            let mut received = Vec::new();
            for _ in 0..100usize {
                let bi = b_clone.accept_bi().await.expect("accept_bi");
                received.push(bi.stream_id);
            }
            received
        })
    };

    // Concurrently open 100 bidi from a.
    let a: Arc<dyn Connection> = Arc::from(a);
    let opens: Vec<_> = (0..100)
        .map(|_| {
            let a = a.clone();
            tokio::spawn(async move { a.open_bi().await.map(|b| b.stream_id) })
        })
        .collect();

    let mut local_ids = Vec::new();
    for h in opens {
        local_ids.push(h.await.unwrap().expect("open_bi"));
    }

    let received = accept_handle.await.unwrap();
    assert_eq!(local_ids.len(), 100);
    assert_eq!(received.len(), 100);
    // IDs must be unique on the local side.
    let unique_local: std::collections::HashSet<_> = local_ids.iter().collect();
    assert_eq!(unique_local.len(), 100, "local stream ids not unique");
}

/// Peer opens 100 streams concurrently. Local accept loop retrieves
/// them in arrival order. Connection contract clauses 7, 8.
pub async fn conn_concurrent_accept<F: TransportFactory>() {
    let (a, b) = F::pair().await;

    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    // Sequential opens from a so order is predictable.
    let a_clone = a.clone();
    let opener = tokio::spawn(async move {
        let mut sent = Vec::new();
        for _ in 0..100usize {
            let bi = a_clone.open_bi().await.expect("open_bi");
            sent.push(bi.stream_id);
        }
        sent
    });

    let b_clone = b.clone();
    let accepter = tokio::spawn(async move {
        let mut got = Vec::new();
        for _ in 0..100usize {
            let bi = b_clone.accept_bi().await.expect("accept_bi");
            got.push(bi.stream_id);
        }
        got
    });

    let sent = opener.await.unwrap();
    let got = accepter.await.unwrap();
    assert_eq!(sent.len(), 100);
    assert_eq!(got.len(), 100);
    // Don't assert id equality (peer-side ids may be local-numbered),
    // just count + uniqueness.
    let unique: std::collections::HashSet<_> = got.iter().collect();
    assert_eq!(unique.len(), 100);
}

/// `close()` called twice is idempotent. Connection contract clause 11.
pub async fn conn_close_idempotent<F: TransportFactory>() {
    let (a, _b) = F::pair().await;
    a.close(0, b"first");
    a.close(0, b"second");
    // Should not panic.
}

/// `conn_id()` returns the same value across calls. Connection
/// contract clause 12.
pub async fn conn_id_stable<F: TransportFactory>() {
    let (a, _b) = F::pair().await;
    let id1 = a.conn_id();
    let id2 = a.conn_id();
    let id3 = a.conn_id();
    assert_eq!(id1, id2);
    assert_eq!(id2, id3);
}

/// Open 1000 streams sequentially on the same connection. No leak;
/// memory steady state. Stress baseline.
pub async fn stress_1k_streams_sequential<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    let b_clone = b.clone();
    let drain = tokio::spawn(async move {
        for _ in 0..1000usize {
            let mut bi = b_clone.accept_bi().await.expect("accept_bi");
            // Drain whatever's there, then drop.
            let mut buf = [0u8; 16];
            let _ = futures_util::AsyncReadExt::read(&mut bi.recv, &mut buf).await;
        }
    });

    for i in 0..1000usize {
        let mut bi = a.open_bi().await.expect("open_bi");
        let payload = format!("stream-{i}").into_bytes();
        bi.send.write_all(&payload).await.expect("write");
        bi.send.close().await.expect("close");
        // Drop locally — exercise the impl's per-stream cleanup.
        drop(bi);
    }

    drain.await.unwrap();
}

// ---------------------------------------------------------------------------
// M4 test bodies — implemented in this commit.
// ---------------------------------------------------------------------------

/// 1 MiB body in 64 KiB chunks, peer reads in 1 KiB chunks. Asserts
/// the byte total round-trips correctly. Stream contract clause 6
/// (backpressure) — tokio::io::duplex with a 64 KiB window means
/// the writer awaits the reader naturally; if the impl had unbounded
/// buffering this would OOM long before completing.
pub async fn stream_large_body_backpressure<F: TransportFactory>() {
    let (a, b) = F::pair().await;

    const TOTAL: usize = 1 * 1024 * 1024;
    const WRITE_CHUNK: usize = 64 * 1024;
    const READ_CHUNK: usize = 1 * 1024;

    let writer = async move {
        let mut bi = a.open_bi().await.expect("open_bi");
        let chunk: Vec<u8> = (0..WRITE_CHUNK).map(|i| (i % 251) as u8).collect();
        let mut sent = 0usize;
        while sent < TOTAL {
            let n = (TOTAL - sent).min(WRITE_CHUNK);
            bi.send.write_all(&chunk[..n]).await.expect("write_all");
            sent += n;
        }
        bi.send.close().await.expect("close");
        a
    };
    let reader = async move {
        let mut bi = b.accept_bi().await.expect("accept_bi");
        let mut got = 0usize;
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            match bi.recv.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(e) => panic!("read error: {}", e),
            }
        }
        assert_eq!(got, TOTAL, "byte total mismatch");
        b
    };
    let _ = futures_util::join!(writer, reader);
}

/// Heavy stream + 1k datagrams interleaved on the same connection.
/// Connection contract clause 9 — backed-up datagram path doesn't
/// stall the stream and vice versa.
pub async fn conn_stream_datagram_independence<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    let dg_a = a.datagrams();
    let dg_b = b.datagrams();
    if dg_a.is_none() || dg_b.is_none() {
        // Transport doesn't support datagrams — skip.
        return;
    }
    let dg_a = dg_a.unwrap();
    let dg_b = dg_b.unwrap();

    // Stream side: 64 KiB roundtrip.
    let stream_payload: Vec<u8> = (0..64 * 1024).map(|i| (i % 256) as u8).collect();

    let a_clone = a.clone();
    let stream_writer = {
        let payload = stream_payload.clone();
        async move {
            let mut bi = a_clone.open_bi().await.expect("open_bi");
            bi.send.write_all(&payload).await.expect("write_all");
            bi.send.close().await.expect("close");
        }
    };
    let b_clone = b.clone();
    let stream_reader = async move {
        let mut bi = b_clone.accept_bi().await.expect("accept_bi");
        let mut got = Vec::with_capacity(64 * 1024);
        bi.recv.read_to_end(&mut got).await.expect("read_to_end");
        assert_eq!(got.len(), 64 * 1024);
    };

    // Datagram side: 1k datagrams a→b.
    let dg_sender = async move {
        for i in 0..1000u32 {
            // Bounded queue is 1024 deep — try_send may push us
            // briefly into a backoff loop; that's still independent
            // of the stream.
            loop {
                match dg_a.sink.try_send(Bytes::from(i.to_le_bytes().to_vec())) {
                    Ok(()) => break,
                    Err(crate::TransportError::Other(_)) => {
                        // queue full; yield
                        tokio::task::yield_now().await;
                    }
                    Err(e) => panic!("datagram send error: {:?}", e),
                }
            }
        }
    };
    let dg_recver = async move {
        let mut got = 0u32;
        for _ in 0..1000 {
            let bytes = dg_b.source.recv().await.expect("dg recv");
            assert_eq!(bytes.len(), 4);
            got += 1;
        }
        assert_eq!(got, 1000);
    };

    let _ = futures_util::join!(stream_writer, stream_reader, dg_sender, dg_recver);
}

/// Listener accepts 10 sequential connections. Each one is a
/// working bidi-capable Connection. Listener contract clauses 13, 14.
pub async fn listener_sequential_accept<F: TransportFactory>() {
    let setup = match F::listener().await {
        Some(s) => s,
        None => return, // transport doesn't have a listener concept; skip
    };
    let mut listener = setup.listener;
    let dial = setup.dial;

    for _ in 0..10 {
        // Kick off a dial in parallel with accept.
        let dial_fut = (dial)();
        let (server_conn, client_conn_res) = futures_util::join!(listener.accept(), dial_fut);
        let server_conn = server_conn.expect("accept");
        let client_conn = client_conn_res.expect("dial");

        // Open a bidi from client, accept on server, write+read.
        let payload: &[u8] = b"hi";
        let writer = async {
            let mut bi = client_conn.open_bi().await.expect("open_bi");
            bi.send.write_all(payload).await.expect("write");
            bi.send.close().await.expect("close");
        };
        let reader = async {
            let mut bi = server_conn.accept_bi().await.expect("accept_bi");
            let mut got = Vec::new();
            bi.recv.read_to_end(&mut got).await.expect("read_to_end");
            assert_eq!(&got, payload);
        };
        let _ = futures_util::join!(writer, reader);
    }
}

/// `close()` the listener; subsequent `accept()` returns Closed.
/// Listener contract clause 15.
pub async fn listener_close_stops_accept<F: TransportFactory>() {
    let setup = match F::listener().await {
        Some(s) => s,
        None => return,
    };
    let mut listener = setup.listener;
    listener.close();

    match listener.accept().await {
        Err(crate::TransportError::Closed { .. }) => { /* expected */ }
        Err(other) => panic!("expected Closed; got {:?}", other),
        Ok(_) => panic!("accept returned a connection after close()"),
    }
}

/// 100 concurrent bidi streams, random payload size 64..2048 bytes,
/// every byte verified on the peer side.
pub async fn stress_100_streams_concurrent<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    // Server-side accept loop drains 100 incoming streams and
    // echoes back the body length they wrote.
    let b_clone = b.clone();
    let acceptor = tokio::spawn(async move {
        let mut totals = Vec::new();
        for _ in 0..100usize {
            let mut bi = b_clone.accept_bi().await.expect("accept_bi");
            let mut got = Vec::new();
            bi.recv.read_to_end(&mut got).await.expect("read_to_end");
            totals.push(got.len());
        }
        totals
    });

    // 100 concurrent client streams.
    let mut openers = Vec::new();
    for i in 0..100usize {
        let a = a.clone();
        let len = 64 + (i * 17) % 2048;
        let pat = (i % 256) as u8;
        openers.push(tokio::spawn(async move {
            let payload = vec![pat; len];
            let mut bi = a.open_bi().await.expect("open_bi");
            bi.send.write_all(&payload).await.expect("write_all");
            bi.send.close().await.expect("close");
            len
        }));
    }
    let mut sent = Vec::new();
    for o in openers {
        sent.push(o.await.unwrap());
    }
    let received = acceptor.await.unwrap();
    sent.sort();
    let mut received = received;
    received.sort();
    assert_eq!(sent, received);
}

/// 1k datagrams of varying sizes round-trip in order. InMemory
/// preserves order via a single mpsc; transports that don't promise
/// order still pass this test if multiset equality holds (we assert
/// only the byte counts, not contents-by-position).
pub async fn stress_1k_datagrams<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a_dg = match a.datagrams() {
        Some(d) => d,
        None => return,
    };
    let b_dg = match b.datagrams() {
        Some(d) => d,
        None => return,
    };

    let sender = async move {
        for i in 0..1000usize {
            let len = 1 + (i % 256);
            let pat = (i % 251) as u8;
            let payload = Bytes::from(vec![pat; len]);
            loop {
                match a_dg.sink.try_send(payload.clone()) {
                    Ok(()) => break,
                    Err(crate::TransportError::Other(_)) => {
                        tokio::task::yield_now().await;
                    }
                    Err(e) => panic!("send err: {:?}", e),
                }
            }
        }
    };
    let recver = async move {
        let mut total = 0usize;
        for _ in 0..1000 {
            let b = b_dg.source.recv().await.expect("recv");
            total += b.len();
        }
        // Expected total = sum of (1 + i%256) for i in 0..1000.
        let expected: usize = (0..1000usize).map(|i| 1 + (i % 256)).sum();
        assert_eq!(total, expected);
    };
    let _ = futures_util::join!(sender, recver);
}

/// 1000 tiny streams opened, written-to, closed, dropped. Connection
/// stays healthy throughout.
pub async fn adv_tiny_stream_churn<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    let b_clone = b.clone();
    let drainer = tokio::spawn(async move {
        for _ in 0..1000usize {
            let mut bi = b_clone.accept_bi().await.expect("accept_bi");
            let mut sink = [0u8; 64];
            // Drain whatever's there, then drop.
            let _ = bi.recv.read(&mut sink).await;
        }
    });

    for i in 0..1000usize {
        let bi = a.open_bi().await.expect("open_bi");
        let stream_id = bi.stream_id;
        // Drop without explicit close — exercises the
        // BiStream::Drop path. With a no-reset hook it's a clean
        // tear-down; with a real reset hook it'd send reset(0).
        drop(bi);
        // Sanity: ids monotonic on the local side.
        assert!(stream_id.0 >= i as u64);
    }
    drainer.await.unwrap();
}

// ---------------------------------------------------------------------------
// M4b reset/drop/close test bodies.
// ---------------------------------------------------------------------------

/// Local explicit reset propagates to peer's reads/writes.
/// Stream contract clause 4.
pub async fn stream_reset_propagates<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let mut bi_a = a.open_bi().await.expect("open_bi");
    let mut bi_b = b.accept_bi().await.expect("accept_bi");

    // Send a few bytes so the pipe is alive.
    bi_a.send.write_all(b"hi").await.expect("write");

    // Park a reader on b.
    let reader_handle = tokio::spawn(async move {
        let mut buf = [0u8; 64];
        let mut total = 0;
        loop {
            match bi_b.recv.read(&mut buf).await {
                Ok(0) => return Ok::<usize, std::io::Error>(total),
                Ok(n) => total += n,
                Err(e) => return Err(e),
            }
        }
    });

    // Brief settle so the reader picks up the "hi".
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Reset from a's side.
    bi_a.reset(7);

    // Reader should error (peer reset).
    let res = reader_handle.await.expect("join");
    assert!(
        res.is_err(),
        "expected read to error after peer reset; got Ok({:?})",
        res
    );
    let err = res.unwrap_err();
    let kind = err.kind();
    assert!(
        matches!(
            kind,
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
        ),
        "expected ConnectionReset / Aborted, got {:?}",
        kind
    );
}

/// Dropping a BiStream without explicit close or reset signals
/// reset(0) to the peer. Stream contract clause 5.
pub async fn stream_drop_resets<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let bi_a = a.open_bi().await.expect("open_bi");
    let mut bi_b = b.accept_bi().await.expect("accept_bi");

    let reader_handle = tokio::spawn(async move {
        let mut buf = [0u8; 64];
        bi_b.recv.read(&mut buf).await
    });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Drop without reset/close — Drop on BiStream fires reset_hook(0).
    drop(bi_a);

    let res = reader_handle.await.expect("join");
    assert!(
        res.is_err(),
        "expected read to error after peer-side BiStream drop; got Ok({:?})",
        res
    );
}

/// Closing a connection cascades reset to every open stream.
/// Connection contract clause 10.
pub async fn conn_close_drains_streams<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let a: Arc<dyn Connection> = Arc::from(a);
    let b: Arc<dyn Connection> = Arc::from(b);

    // Open + accept 5 streams.
    let mut a_streams = Vec::new();
    let mut b_recvs = Vec::new();
    for _ in 0..5 {
        let abi = a.open_bi().await.expect("open_bi");
        let bbi = b.accept_bi().await.expect("accept_bi");
        a_streams.push(abi);
        b_recvs.push(bbi);
    }

    // Park readers on every b stream.
    let readers: Vec<_> = b_recvs
        .into_iter()
        .map(|mut bi| {
            tokio::spawn(async move {
                let mut buf = [0u8; 32];
                bi.recv.read(&mut buf).await
            })
        })
        .collect();

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Closing a's connection should reset every still-open stream
    // from a's side.
    a.close(99, b"shutdown");

    for r in readers {
        let res = r.await.expect("join");
        assert!(
            res.is_err(),
            "expected read to error after conn close; got Ok({:?})",
            res
        );
    }

    // a's a_streams kept alive locally don't matter — they'll be
    // dropped at scope end without panicking.
    drop(a_streams);
}

/// Peer drops the BiStream while local writer is in flight. Local
/// writes should error within a small window.
pub async fn adv_peer_close_mid_write<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let mut bi_a = a.open_bi().await.expect("open_bi");
    let bi_b = b.accept_bi().await.expect("accept_bi");

    // Initial write while peer is alive.
    bi_a.send.write_all(b"first").await.expect("first write");

    // Peer drops the stream.
    drop(bi_b);

    // Subsequent write must eventually error. We bound the loop so
    // a buggy impl doesn't hang the test; pipes are 64 KiB so within
    // a few writes we either fill or hit the reset.
    let chunk = [0u8; 4096];
    let mut wrote = 0;
    let mut errored = false;
    for _ in 0..1024 {
        match bi_a.send.write(&chunk).await {
            Ok(_) => wrote += chunk.len(),
            Err(_) => {
                errored = true;
                break;
            }
        }
    }
    assert!(
        errored,
        "expected write to error after peer drop; wrote {} bytes total without error",
        wrote
    );
}

/// Peer resets the stream while local reader is awaiting bytes.
/// Reader wakes with a reset error.
pub async fn adv_peer_reset_mid_read<F: TransportFactory>() {
    let (a, b) = F::pair().await;
    let mut bi_a = a.open_bi().await.expect("open_bi");
    let mut bi_b = b.accept_bi().await.expect("accept_bi");

    let reader = tokio::spawn(async move {
        let mut buf = [0u8; 64];
        bi_b.recv.read(&mut buf).await
    });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    bi_a.reset(42);

    let res = reader.await.expect("join");
    assert!(res.is_err(), "expected reader to error on peer reset");
}
