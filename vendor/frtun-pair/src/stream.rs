//! `PairStream` — `AsyncRead + AsyncWrite` over a WebSocket binary
//! channel, post-handshake.
//!
//! After the handshake (Listen/Dial → Ready/Dialed/Incoming) the
//! bridge forwards raw binary WebSocket frames verbatim between the
//! two paired peers. We expose that as an AsyncRead+AsyncWrite stream
//! so consumers can layer their own framing (ChaCha20-Poly1305
//! envelopes, length-prefixed JSON, whatever) without caring that the
//! underlying carrier happens to be WebSocket-over-WSS.
//!
//! Implementation: an actor task owns the inner `BinaryDuplex`
//! transport. The `PairStream` holds two mpsc channels — bytes get
//! pushed in via `AsyncWrite::poll_write`, the actor pops them off
//! and calls `inner.send_binary`. The actor reads `inner.recv_binary`
//! in a loop and pushes the bytes through the other mpsc to the
//! `AsyncRead::poll_read` side. No `unsafe`, no lifetime gymnastics.

use bytes::{Bytes, BytesMut};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;

/// A bidirectional channel of binary WebSocket-like frames.
///
/// Native uses tokio-tungstenite's `WebSocketStream`, tests use a
/// `tokio::sync::mpsc`-backed mock, the wasm port will use the
/// browser WebSocket via web-sys. All three plug in here.
#[async_trait::async_trait]
pub trait BinaryDuplex: Send + 'static {
    /// Send one binary frame.
    async fn send_binary(&mut self, data: Bytes) -> io::Result<()>;
    /// Receive one binary frame. `Ok(None)` means the peer closed.
    async fn recv_binary(&mut self) -> io::Result<Option<Bytes>>;
    /// Send a text frame (handshake JSON).
    async fn send_text(&mut self, text: String) -> io::Result<()>;
    /// Receive one frame as text. Returns None on close.
    async fn recv_text(&mut self) -> io::Result<Option<String>>;
    /// Best-effort close.
    async fn close(&mut self) -> io::Result<()>;
}

/// The post-handshake bidirectional byte stream.
#[derive(Debug)]
pub struct PairStream {
    /// Bytes the actor has handed us, awaiting poll_read consumption.
    rx:           mpsc::UnboundedReceiver<io::Result<Bytes>>,
    /// Bytes poll_write enqueues for the actor to send.
    tx:           mpsc::UnboundedSender<Bytes>,
    /// Leftover from the last frame after a partial read.
    leftover:     BytesMut,
    /// True once the actor has signalled EOF on the rx side.
    eof:          bool,
    /// Pre-handshake remote peer name.
    remote_peer:  String,
}

impl PairStream {
    pub(crate) fn spawn<D: BinaryDuplex>(inner: D, remote_peer: String) -> Self {
        let (rx_in,  rx_out) = mpsc::unbounded_channel::<io::Result<Bytes>>();
        let (tx_in,  tx_out) = mpsc::unbounded_channel::<Bytes>();
        tokio::spawn(actor::<D>(inner, rx_in, tx_out));
        Self {
            rx: rx_out,
            tx: tx_in,
            leftover: BytesMut::new(),
            eof: false,
            remote_peer,
        }
    }

    pub fn remote_peer(&self) -> &str {
        &self.remote_peer
    }
}

/// Actor — owns the inner duplex; relays bytes both directions.
async fn actor<D: BinaryDuplex>(
    mut inner: D,
    rx_in: mpsc::UnboundedSender<io::Result<Bytes>>,
    mut tx_out: mpsc::UnboundedReceiver<Bytes>,
) {
    loop {
        tokio::select! {
            biased;
            outbound = tx_out.recv() => {
                match outbound {
                    Some(bytes) => {
                        if let Err(e) = inner.send_binary(bytes).await {
                            let _ = rx_in.send(Err(e));
                            break;
                        }
                    }
                    None => {
                        // Tx side dropped; close the inner.
                        let _ = inner.close().await;
                        break;
                    }
                }
            }
            inbound = inner.recv_binary() => {
                match inbound {
                    Ok(Some(bytes)) => {
                        if rx_in.send(Ok(bytes)).is_err() {
                            // Rx side dropped — peer doesn't care anymore.
                            let _ = inner.close().await;
                            break;
                        }
                    }
                    Ok(None) => {
                        // EOF — send an empty Ok to signal close.
                        let _ = rx_in.send(Ok(Bytes::new()));
                        break;
                    }
                    Err(e) => {
                        let _ = rx_in.send(Err(e));
                        break;
                    }
                }
            }
        }
    }
}

// --- AsyncRead -------------------------------------------------------

impl AsyncRead for PairStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx:  &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Drain leftover from a previous partial read.
        if !self.leftover.is_empty() {
            let to_copy = std::cmp::min(self.leftover.len(), buf.remaining());
            let bytes   = self.leftover.split_to(to_copy);
            buf.put_slice(&bytes);
            return Poll::Ready(Ok(()));
        }
        if self.eof {
            // EOF: leave buf.filled() at 0 to signal close.
            return Poll::Ready(Ok(()));
        }
        match self.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                // Actor went away without an explicit EOF — surface
                // close.
                self.eof = true;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(e)),
            Poll::Ready(Some(Ok(bytes))) => {
                if bytes.is_empty() {
                    // Sentinel for EOF from actor.
                    self.eof = true;
                    return Poll::Ready(Ok(()));
                }
                let to_copy = std::cmp::min(bytes.len(), buf.remaining());
                buf.put_slice(&bytes[..to_copy]);
                if to_copy < bytes.len() {
                    self.leftover.extend_from_slice(&bytes[to_copy..]);
                }
                Poll::Ready(Ok(()))
            }
        }
    }
}

// --- AsyncWrite ------------------------------------------------------

impl AsyncWrite for PairStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // mpsc::UnboundedSender::send is sync. If it errors the actor is
        // gone — that's BrokenPipe.
        match self.tx.send(Bytes::copy_from_slice(buf)) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(_) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe, "pair-stream actor closed"))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        // Drop the tx channel — actor will see `None` and close inner.
        // We replace self.tx with a closed sender so any later poll_write
        // returns BrokenPipe cleanly.
        let (closed_tx, closed_rx) = mpsc::unbounded_channel();
        drop(closed_rx);   // close immediately
        self.tx = closed_tx;
        Poll::Ready(Ok(()))
    }
}

// --- In-process mock duplex for tests --------------------------------

#[cfg(test)]
pub(crate) mod mock {
    use super::*;

    #[derive(Debug, Clone)]
    pub enum MockFrame {
        Binary(Bytes),
        Text(String),
        Close,
    }

    pub struct MockDuplex {
        pub tx: mpsc::UnboundedSender<MockFrame>,
        pub rx: mpsc::UnboundedReceiver<MockFrame>,
    }

    pub fn pair() -> (MockDuplex, MockDuplex) {
        let (a_tx, b_rx) = mpsc::unbounded_channel();
        let (b_tx, a_rx) = mpsc::unbounded_channel();
        (MockDuplex { tx: a_tx, rx: a_rx }, MockDuplex { tx: b_tx, rx: b_rx })
    }

    #[async_trait::async_trait]
    impl BinaryDuplex for MockDuplex {
        async fn send_binary(&mut self, data: Bytes) -> io::Result<()> {
            self.tx.send(MockFrame::Binary(data))
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
        }
        async fn recv_binary(&mut self) -> io::Result<Option<Bytes>> {
            match self.rx.recv().await {
                Some(MockFrame::Binary(b)) => Ok(Some(b)),
                Some(MockFrame::Text(_))   => Err(io::Error::new(
                    io::ErrorKind::InvalidData, "unexpected text frame")),
                Some(MockFrame::Close) | None => Ok(None),
            }
        }
        async fn send_text(&mut self, text: String) -> io::Result<()> {
            self.tx.send(MockFrame::Text(text))
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
        }
        async fn recv_text(&mut self) -> io::Result<Option<String>> {
            match self.rx.recv().await {
                Some(MockFrame::Text(s)) => Ok(Some(s)),
                Some(MockFrame::Binary(_)) => Err(io::Error::new(
                    io::ErrorKind::InvalidData, "unexpected binary frame")),
                Some(MockFrame::Close) | None => Ok(None),
            }
        }
        async fn close(&mut self) -> io::Result<()> {
            let _ = self.tx.send(MockFrame::Close);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn loopback_send_receive() {
        let (a, b) = mock::pair();
        let mut alice = PairStream::spawn(a, "frtun1bob.peer".into());
        let mut bob   = PairStream::spawn(b, "frtun1alice.peer".into());

        alice.write_all(b"hello bob").await.unwrap();
        let mut buf = [0u8; 9];
        bob.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello bob");

        bob.write_all(b"hi alice").await.unwrap();
        let mut buf = [0u8; 8];
        alice.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi alice");

        assert_eq!(alice.remote_peer(), "frtun1bob.peer");
        assert_eq!(bob.remote_peer(),   "frtun1alice.peer");
    }

    #[tokio::test]
    async fn partial_read_drains_leftover() {
        let (a, b) = mock::pair();
        let mut alice = PairStream::spawn(a, "frtun1bob.peer".into());
        let mut bob   = PairStream::spawn(b, "frtun1alice.peer".into());

        alice.write_all(b"0123456789ABCDEF").await.unwrap();
        let mut buf = [0u8; 8];
        bob.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"01234567");
        bob.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"89ABCDEF");
    }

    #[tokio::test]
    async fn shutdown_surfaces_eof_on_peer_read() {
        let (a, b) = mock::pair();
        let mut alice = PairStream::spawn(a, "frtun1bob.peer".into());
        let mut bob   = PairStream::spawn(b, "frtun1alice.peer".into());

        alice.shutdown().await.unwrap();
        // Give the actor a beat to drain + close.
        tokio::task::yield_now().await;
        let mut buf = [0u8; 4];
        let n = bob.read(&mut buf).await.unwrap();
        assert_eq!(n, 0, "expected EOF after peer shutdown");
    }

    #[tokio::test]
    async fn back_to_back_writes_arrive_in_order() {
        let (a, b) = mock::pair();
        let mut alice = PairStream::spawn(a, "frtun1bob.peer".into());
        let mut bob   = PairStream::spawn(b, "frtun1alice.peer".into());

        // Burst three writes without awaiting; the actor's mpsc should
        // preserve order.
        alice.write_all(b"one").await.unwrap();
        alice.write_all(b"two").await.unwrap();
        alice.write_all(b"three").await.unwrap();

        let mut buf = [0u8; 11];
        bob.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"onetwothree");
    }
}
