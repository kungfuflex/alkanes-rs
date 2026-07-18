//! In-memory map `peer_name → oneshot::Sender<DialRequest>`. A
//! listener registers itself with a fresh oneshot receiver; a dialer
//! looks up the listener's sender and ships its half of the bridged
//! connection through it. The registry removes the entry on dial so
//! each Listen handles exactly one Incoming.
//!
//! For one-shot pair flows (which is the only use case today) the
//! receive-once semantics are ideal — they prevent a malicious party
//! who learns the bech32 from squatting on the listener slot.
//! Future "session-keepalive" mode (mobile-stays-paired-after-cli-exit)
//! would need a multi-shot variant; not relevant for v1.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

/// What gets handed across the registry from a Listen-side to the
/// matching Dial-side: the dialer's peer name + a oneshot the listener
/// will use to send back its "accepted" handle.
#[derive(Debug)]
pub struct DialNotice {
    pub dialer_name: String,
    /// The dialer's connection-half handle that the listener will
    /// receive (impl-defined; the server.rs wires it to the WS sink).
    pub dialer_handle: ConnHandle,
}

/// Opaque connection handle the bridge uses to plug two WebSockets
/// together. Concretely, it's a pair of mpsc channels — one direction
/// of binary frames each — that the server.rs forwarder task drains.
#[derive(Debug)]
pub struct ConnHandle {
    pub tx: tokio::sync::mpsc::UnboundedSender<bytes::Bytes>,
    pub rx: tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>,
}

#[derive(Default, Clone)]
pub struct Registry {
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<DialNotice>>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a listener. Returns the receiver the listener should
    /// await for an inbound dial. If a previous listener was
    /// registered under the same peer name, this REPLACES it — the
    /// old listener's oneshot is dropped (it'll surface a
    /// "listener canceled" close when its handshake half closes).
    pub fn register(&self, peer_name: String) -> oneshot::Receiver<DialNotice> {
        let (tx, rx) = oneshot::channel();
        let mut guard = self.inner.lock();
        guard.insert(peer_name, tx);
        rx
    }

    /// Attempt to dial. Returns `Ok(())` on success (notice delivered
    /// to the registered listener) or `Err(DialError)` describing why.
    pub fn dial(&self, target: &str, notice: DialNotice) -> Result<(), DialError> {
        let mut guard = self.inner.lock();
        let tx = guard.remove(target).ok_or(DialError::PeerNotFound)?;
        drop(guard);
        tx.send(notice).map_err(|_| DialError::PeerGone)
    }

    /// Drop a listener registration explicitly (e.g. WS closed before
    /// any dial arrived).
    pub fn deregister(&self, peer_name: &str) {
        self.inner.lock().remove(peer_name);
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DialError {
    #[error("no listener registered for that peer name")]
    PeerNotFound,
    #[error("listener went away before the notice could be delivered")]
    PeerGone,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn mk_handle() -> ConnHandle {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_tx, rx) = mpsc::unbounded_channel();
        ConnHandle { tx, rx }
    }

    #[tokio::test]
    async fn register_then_dial_delivers_notice() {
        let r = Registry::new();
        let mut rx = r.register("frtun1bob.peer".into());
        r.dial("frtun1bob.peer", DialNotice {
            dialer_name:   "frtun1alice.peer".into(),
            dialer_handle: mk_handle(),
        }).unwrap();
        let notice = rx.try_recv().unwrap();
        assert_eq!(notice.dialer_name, "frtun1alice.peer");
        assert!(r.is_empty(), "registry empties on dial");
    }

    #[tokio::test]
    async fn dial_with_no_listener_errors_peer_not_found() {
        let r = Registry::new();
        let err = r.dial("frtun1ghost.peer", DialNotice {
            dialer_name:   "frtun1alice.peer".into(),
            dialer_handle: mk_handle(),
        }).unwrap_err();
        assert!(matches!(err, DialError::PeerNotFound));
    }

    #[tokio::test]
    async fn second_register_replaces_first() {
        let r = Registry::new();
        let _rx1 = r.register("frtun1bob.peer".into());
        let mut rx2 = r.register("frtun1bob.peer".into());
        r.dial("frtun1bob.peer", DialNotice {
            dialer_name:   "frtun1alice.peer".into(),
            dialer_handle: mk_handle(),
        }).unwrap();
        // The notice goes to rx2, not rx1 (rx1's tx was dropped).
        let notice = rx2.try_recv().unwrap();
        assert_eq!(notice.dialer_name, "frtun1alice.peer");
    }

    #[tokio::test]
    async fn deregister_clears_slot() {
        let r = Registry::new();
        let _rx = r.register("frtun1bob.peer".into());
        r.deregister("frtun1bob.peer");
        let err = r.dial("frtun1bob.peer", DialNotice {
            dialer_name:   "frtun1alice.peer".into(),
            dialer_handle: mk_handle(),
        }).unwrap_err();
        assert!(matches!(err, DialError::PeerNotFound));
    }
}
