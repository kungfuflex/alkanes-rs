//! Contract tests for `tlsfetch-events`.
//!
//! Asserts:
//!   1. Default ProfilerHandle is no-op.
//!   2. NoOpProfiler reports inactive; Timeline reports active.
//!   3. Events emitted via a Timeline-backed handle land in the
//!      timeline's snapshot in arrival order.
//!   4. Timestamps are monotonically non-decreasing.
//!   5. clear() resets the timeline; subsequent emissions land
//!      after with a fresh t=0.
//!   6. filter_name() returns only matching events.
//!   7. render() produces non-empty ASCII output for a non-empty
//!      timeline; empty timeline renders as "(empty timeline)".
//!   8. Multiple ProfilerHandle clones share one backing Arc — i.e.
//!      emissions from any clone land in the same timeline.
//!   9. The handle is Send + Sync (compile-time check below).

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tlsfetch_events::{
    timeline, Event, Initiator, NoOpProfiler, Profiler, ProfilerHandle, StreamCloseKind,
    StreamDir, Timeline,
};

#[test]
fn default_handle_is_noop() {
    let h = ProfilerHandle::default();
    assert!(!h.is_active(), "default handle should report inactive");
    // emit something; it should be a silent drop, no panic.
    h.emit(Event::Custom {
        name: "ignored".into(),
        data: None,
    });
}

#[test]
fn noop_reports_inactive() {
    let p = NoOpProfiler;
    assert!(!p.is_active());
}

#[test]
fn timeline_reports_active() {
    let tl = Timeline::new();
    assert!(tl.is_active());
}

#[test]
fn emits_land_in_order() {
    let (tl, h) = timeline();
    h.emit(Event::Connecting {
        conn_id: 1,
        target: "x".into(),
    });
    h.emit(Event::Connected {
        conn_id: 1,
        peer_addr: None,
        alpn: None,
    });
    h.emit(Event::StreamOpened {
        conn_id: 1,
        stream_id: 7,
        dir: StreamDir::Bi,
        initiator: Initiator::Local,
    });
    let snap = tl.snapshot();
    assert_eq!(snap.len(), 3);
    assert_eq!(snap[0].event.name(), "connecting");
    assert_eq!(snap[1].event.name(), "connected");
    assert_eq!(snap[2].event.name(), "stream_opened");
}

#[test]
fn timestamps_monotonic() {
    let (tl, h) = timeline();
    for i in 0..50 {
        h.emit(Event::StreamWrite {
            conn_id: 1,
            stream_id: 1,
            bytes: i,
        });
        // Tiny sleep so the wall clock visibly advances. SystemTime
        // is allowed to go backwards under e.g. NTP slew, so the
        // assertion is non-decreasing rather than strictly increasing.
        thread::sleep(Duration::from_micros(50));
    }
    let snap = tl.snapshot();
    let millis: Vec<u64> = snap.iter().map(|t| t.millis()).collect();
    let mut last = 0u64;
    for m in millis {
        assert!(m >= last, "timestamp went backwards: {} < {}", m, last);
        last = m;
    }
}

#[test]
fn clear_resets_timeline() {
    let (tl, h) = timeline();
    for _ in 0..10 {
        h.emit(Event::Custom {
            name: "tick".into(),
            data: None,
        });
    }
    assert_eq!(tl.len(), 10);
    tl.clear();
    assert!(tl.is_empty());
    h.emit(Event::Custom {
        name: "after-clear".into(),
        data: None,
    });
    assert_eq!(tl.len(), 1);
    assert_eq!(tl.snapshot()[0].event.name(), "custom");
}

#[test]
fn filter_name_isolates_kind() {
    let (tl, h) = timeline();
    for i in 0..5 {
        h.emit(Event::StreamWrite {
            conn_id: 1,
            stream_id: 1,
            bytes: i,
        });
        h.emit(Event::StreamRead {
            conn_id: 1,
            stream_id: 1,
            bytes: i * 2,
        });
    }
    let writes = tl.filter_name("stream_write");
    let reads = tl.filter_name("stream_read");
    assert_eq!(writes.len(), 5);
    assert_eq!(reads.len(), 5);
    for (i, w) in writes.iter().enumerate() {
        match w.event {
            Event::StreamWrite { bytes, .. } => assert_eq!(bytes as u64, i as u64),
            _ => panic!("filter returned wrong variant"),
        }
    }
}

#[test]
fn render_non_empty_has_content() {
    let (tl, h) = timeline();
    assert_eq!(tl.render(), "(empty timeline)");
    h.emit(Event::Connecting {
        conn_id: 42,
        target: "example.com:443".into(),
    });
    h.emit(Event::Connected {
        conn_id: 42,
        peer_addr: Some("1.2.3.4:443".into()),
        alpn: Some("h3".into()),
    });
    h.emit(Event::RequestStart {
        conn_id: 42,
        stream_id: 4,
        method: "GET".into(),
        path: "/health".into(),
    });
    h.emit(Event::ResponseHeaders {
        conn_id: 42,
        stream_id: 4,
        status: 200,
        content_length: Some(2),
    });
    h.emit(Event::RequestEnd {
        conn_id: 42,
        stream_id: 4,
        status: 200,
        bytes_sent: 64,
        bytes_received: 2,
    });
    let r = tl.render();
    // Sanity: contains every event name and the request endpoint.
    for needle in [
        "connecting",
        "connected",
        "request_start",
        "response_headers",
        "request_end",
        "GET",
        "/health",
        "h3",
    ] {
        assert!(r.contains(needle), "render missing {:?}\n{}", needle, r);
    }
}

#[test]
fn handle_clones_share_backing_timeline() {
    let (tl, h1) = timeline();
    let h2 = h1.clone();
    let h3 = h1.clone();
    h1.emit(Event::Custom {
        name: "from-1".into(),
        data: None,
    });
    h2.emit(Event::Custom {
        name: "from-2".into(),
        data: None,
    });
    h3.emit(Event::Custom {
        name: "from-3".into(),
        data: None,
    });
    let snap = tl.snapshot();
    assert_eq!(snap.len(), 3);
    let names: Vec<&str> = snap
        .iter()
        .map(|t| match &t.event {
            Event::Custom { data: _, name } => name.as_str(),
            _ => "?",
        })
        .collect();
    assert_eq!(names, vec!["from-1", "from-2", "from-3"]);
}

#[test]
fn handle_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let h = ProfilerHandle::noop();
    assert_send_sync(&h);
    // Send across a thread to prove it.
    let h2 = h.clone();
    let join = thread::spawn(move || {
        h2.emit(Event::Custom {
            name: "from-thread".into(),
            data: None,
        });
    });
    join.join().unwrap();
}

#[test]
fn arc_timeline_drops_cleanly() {
    // Make sure the Profiler trait object has the right Drop semantics:
    // dropping all handles + the original Arc should free the timeline.
    let tl = Timeline::new();
    let weak = Arc::downgrade(&tl);
    let handle = ProfilerHandle::from_arc(tl.clone() as Arc<dyn Profiler>);
    drop(handle);
    drop(tl);
    assert!(
        weak.upgrade().is_none(),
        "timeline should be dropped when all refs gone"
    );
}

#[test]
fn concurrent_emit_no_data_race() {
    // Hammer one Timeline from many threads. The mutex inside
    // serializes; this just checks we don't drop events under load.
    let (tl, h) = timeline();
    let mut joins = Vec::new();
    for t in 0..8 {
        let h = h.clone();
        joins.push(thread::spawn(move || {
            for i in 0..1000u64 {
                h.emit(Event::StreamWrite {
                    conn_id: t as u64,
                    stream_id: 1,
                    bytes: i,
                });
            }
        }));
    }
    for j in joins {
        j.join().unwrap();
    }
    assert_eq!(tl.len(), 8 * 1000);
}
