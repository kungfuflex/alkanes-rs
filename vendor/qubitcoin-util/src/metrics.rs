//! Built-in Prometheus metrics for node observability.
//!
//! Bitcoin Core has no built-in metrics, requiring external tools like
//! bitcoin_exporter. We make metrics a first-class feature, providing:
//!
//! - Block processing metrics (height, validation time, connect time)
//! - P2P networking metrics (peer count, messages sent/received, bandwidth)
//! - Mempool metrics (size, bytes, fee rates)
//! - UTXO cache metrics (size, hit rate, flush time)
//! - RPC metrics (request count, latency histogram)
//!
//! All metrics are exposed via a /metrics HTTP endpoint in Prometheus
//! exposition format.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Metric types
// ---------------------------------------------------------------------------

/// A monotonically increasing counter.
#[derive(Debug)]
pub struct Counter {
    value: AtomicU64,
    name: String,
    help: String,
}

impl Counter {
    /// Create a new counter with the given Prometheus metric name and help text.
    pub fn new(name: &str, help: &str) -> Self {
        Counter {
            value: AtomicU64::new(0),
            name: name.to_string(),
            help: help.to_string(),
        }
    }

    /// Increment the counter by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the counter by `v`.
    pub fn inc_by(&self, v: u64) {
        self.value.fetch_add(v, Ordering::Relaxed);
    }

    /// Read the current counter value.
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Render this counter in Prometheus exposition format.
    pub fn format_prometheus(&self) -> String {
        format!(
            "# HELP {} {}\n# TYPE {} counter\n{} {}\n",
            self.name,
            self.help,
            self.name,
            self.name,
            self.get()
        )
    }
}

/// A gauge that can go up or down.
#[derive(Debug)]
pub struct Gauge {
    value: AtomicI64,
    name: String,
    help: String,
}

impl Gauge {
    /// Create a new gauge with the given Prometheus metric name and help text.
    pub fn new(name: &str, help: &str) -> Self {
        Gauge {
            value: AtomicI64::new(0),
            name: name.to_string(),
            help: help.to_string(),
        }
    }

    /// Set the gauge to an absolute value.
    pub fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }

    /// Increment the gauge by 1.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the gauge by 1.
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Read the current gauge value.
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Render this gauge in Prometheus exposition format.
    pub fn format_prometheus(&self) -> String {
        format!(
            "# HELP {} {}\n# TYPE {} gauge\n{} {}\n",
            self.name,
            self.help,
            self.name,
            self.name,
            self.get()
        )
    }
}

/// A histogram that tracks the distribution of observed values.
#[derive(Debug)]
pub struct Histogram {
    name: String,
    help: String,
    buckets: Vec<f64>,
    counts: Vec<AtomicU64>,
    sum: AtomicU64, // stored as bits of f64
    count: AtomicU64,
}

impl Histogram {
    /// Create a new histogram with explicit bucket boundaries.
    pub fn new(name: &str, help: &str, buckets: Vec<f64>) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Histogram {
            name: name.to_string(),
            help: help.to_string(),
            buckets,
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Create with default buckets suitable for latency measurement (seconds).
    pub fn with_default_buckets(name: &str, help: &str) -> Self {
        Self::new(
            name,
            help,
            vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        )
    }

    /// Record an observed value, updating bucket counts, sum, and total count.
    pub fn observe(&self, value: f64) {
        // Update sum
        loop {
            let old = self.sum.load(Ordering::Relaxed);
            let old_f = f64::from_bits(old);
            let new_f = old_f + value;
            if self
                .sum
                .compare_exchange(old, new_f.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        self.count.fetch_add(1, Ordering::Relaxed);

        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Start a timer that observes the elapsed time when dropped.
    pub fn start_timer(&self) -> HistogramTimer<'_> {
        HistogramTimer {
            histogram: self,
            start: Instant::now(),
        }
    }

    /// Render this histogram in Prometheus exposition format.
    pub fn format_prometheus(&self) -> String {
        let mut output = format!(
            "# HELP {} {}\n# TYPE {} histogram\n",
            self.name, self.help, self.name
        );

        for (i, bucket) in self.buckets.iter().enumerate() {
            let count = self.counts[i].load(Ordering::Relaxed);
            output.push_str(&format!(
                "{}_bucket{{le=\"{}\"}} {}\n",
                self.name, bucket, count
            ));
        }

        let total = self.count.load(Ordering::Relaxed);
        output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", self.name, total));

        let sum = f64::from_bits(self.sum.load(Ordering::Relaxed));
        output.push_str(&format!("{}_sum {}\n", self.name, sum));
        output.push_str(&format!("{}_count {}\n", self.name, total));

        output
    }
}

/// Timer guard for histogram observations.
///
/// Records the elapsed wall-clock time as an observation when dropped.
/// Obtain one via [`Histogram::start_timer`].
pub struct HistogramTimer<'a> {
    histogram: &'a Histogram,
    start: Instant,
}

impl<'a> Drop for HistogramTimer<'a> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.histogram.observe(elapsed);
    }
}

// ---------------------------------------------------------------------------
// Node metrics registry
// ---------------------------------------------------------------------------

/// All metrics for a Qubitcoin node.
///
/// Provides counters, gauges, and histograms covering block processing,
/// P2P networking, mempool, UTXO cache, RPC, script verification, and reorgs.
pub struct NodeMetrics {
    // -- Block metrics --

    /// Total number of blocks processed (monotonic counter).
    pub blocks_processed: Counter,
    /// Current best block height.
    pub block_height: Gauge,
    /// Time spent validating blocks (seconds).
    pub block_validation_seconds: Histogram,
    /// Time spent connecting blocks to the active chain (seconds).
    pub block_connect_seconds: Histogram,
    /// Distribution of block sizes in bytes.
    pub block_size_bytes: Histogram,

    // -- P2P metrics --

    /// Number of currently connected peers.
    pub peers_connected: Gauge,
    /// Number of inbound peer connections.
    pub peers_inbound: Gauge,
    /// Number of outbound peer connections.
    pub peers_outbound: Gauge,
    /// Total P2P messages received (monotonic counter).
    pub messages_received: Counter,
    /// Total P2P messages sent (monotonic counter).
    pub messages_sent: Counter,
    /// Total bytes received from peers (monotonic counter).
    pub bytes_received: Counter,
    /// Total bytes sent to peers (monotonic counter).
    pub bytes_sent: Counter,
    /// Total peer disconnections (monotonic counter).
    pub peer_disconnections: Counter,

    // -- Mempool metrics --

    /// Number of transactions currently in the mempool.
    pub mempool_size: Gauge,
    /// Total size of mempool transactions in bytes.
    pub mempool_bytes: Gauge,
    /// Total transactions accepted into the mempool (monotonic counter).
    pub mempool_accepted: Counter,
    /// Total transactions rejected from the mempool (monotonic counter).
    pub mempool_rejected: Counter,

    // -- UTXO cache metrics --

    /// Number of entries in the UTXO cache.
    pub utxo_cache_size: Gauge,
    /// Total UTXO cache hits (monotonic counter).
    pub utxo_cache_hits: Counter,
    /// Total UTXO cache misses (monotonic counter).
    pub utxo_cache_misses: Counter,
    /// Time spent flushing the UTXO cache to disk (seconds).
    pub utxo_flush_seconds: Histogram,

    // -- RPC metrics --

    /// Total RPC requests received (monotonic counter).
    pub rpc_requests: Counter,
    /// Total RPC errors returned (monotonic counter).
    pub rpc_errors: Counter,
    /// Distribution of RPC request latencies (seconds).
    pub rpc_latency_seconds: Histogram,

    // -- Script verification metrics --

    /// Total script verifications performed (monotonic counter).
    pub script_verifications: Counter,
    /// Time spent on script verification (seconds).
    pub script_verification_seconds: Histogram,

    // -- Reorg metrics --

    /// Total chain reorganizations (monotonic counter).
    pub chain_reorgs: Counter,
    /// Distribution of chain reorganization depths (in blocks).
    pub reorg_depth: Histogram,
}

impl NodeMetrics {
    /// Create a new metrics registry with all counters, gauges, and histograms initialized to zero.
    pub fn new() -> Self {
        NodeMetrics {
            // Block metrics
            blocks_processed: Counter::new(
                "qubitcoin_blocks_processed_total",
                "Total number of blocks processed",
            ),
            block_height: Gauge::new("qubitcoin_block_height", "Current best block height"),
            block_validation_seconds: Histogram::with_default_buckets(
                "qubitcoin_block_validation_seconds",
                "Time spent validating blocks",
            ),
            block_connect_seconds: Histogram::with_default_buckets(
                "qubitcoin_block_connect_seconds",
                "Time spent connecting blocks to the chain",
            ),
            block_size_bytes: Histogram::new(
                "qubitcoin_block_size_bytes",
                "Block sizes in bytes",
                vec![
                    1000.0, 10000.0, 100000.0, 500000.0, 1000000.0, 2000000.0, 4000000.0,
                ],
            ),

            // P2P metrics
            peers_connected: Gauge::new("qubitcoin_peers_connected", "Number of connected peers"),
            peers_inbound: Gauge::new(
                "qubitcoin_peers_inbound",
                "Number of inbound peer connections",
            ),
            peers_outbound: Gauge::new(
                "qubitcoin_peers_outbound",
                "Number of outbound peer connections",
            ),
            messages_received: Counter::new(
                "qubitcoin_p2p_messages_received_total",
                "Total P2P messages received",
            ),
            messages_sent: Counter::new(
                "qubitcoin_p2p_messages_sent_total",
                "Total P2P messages sent",
            ),
            bytes_received: Counter::new(
                "qubitcoin_p2p_bytes_received_total",
                "Total bytes received from peers",
            ),
            bytes_sent: Counter::new(
                "qubitcoin_p2p_bytes_sent_total",
                "Total bytes sent to peers",
            ),
            peer_disconnections: Counter::new(
                "qubitcoin_peer_disconnections_total",
                "Total peer disconnections",
            ),

            // Mempool metrics
            mempool_size: Gauge::new(
                "qubitcoin_mempool_size",
                "Number of transactions in the mempool",
            ),
            mempool_bytes: Gauge::new("qubitcoin_mempool_bytes", "Total size of mempool in bytes"),
            mempool_accepted: Counter::new(
                "qubitcoin_mempool_accepted_total",
                "Total transactions accepted to mempool",
            ),
            mempool_rejected: Counter::new(
                "qubitcoin_mempool_rejected_total",
                "Total transactions rejected from mempool",
            ),

            // UTXO cache metrics
            utxo_cache_size: Gauge::new(
                "qubitcoin_utxo_cache_size",
                "Number of entries in the UTXO cache",
            ),
            utxo_cache_hits: Counter::new(
                "qubitcoin_utxo_cache_hits_total",
                "Total UTXO cache hits",
            ),
            utxo_cache_misses: Counter::new(
                "qubitcoin_utxo_cache_misses_total",
                "Total UTXO cache misses",
            ),
            utxo_flush_seconds: Histogram::with_default_buckets(
                "qubitcoin_utxo_flush_seconds",
                "Time spent flushing the UTXO cache",
            ),

            // RPC metrics
            rpc_requests: Counter::new(
                "qubitcoin_rpc_requests_total",
                "Total RPC requests received",
            ),
            rpc_errors: Counter::new("qubitcoin_rpc_errors_total", "Total RPC errors"),
            rpc_latency_seconds: Histogram::with_default_buckets(
                "qubitcoin_rpc_latency_seconds",
                "RPC request latency",
            ),

            // Script verification metrics
            script_verifications: Counter::new(
                "qubitcoin_script_verifications_total",
                "Total script verifications performed",
            ),
            script_verification_seconds: Histogram::with_default_buckets(
                "qubitcoin_script_verification_seconds",
                "Time spent on script verification",
            ),

            // Reorg metrics
            chain_reorgs: Counter::new(
                "qubitcoin_chain_reorgs_total",
                "Total chain reorganizations",
            ),
            reorg_depth: Histogram::new(
                "qubitcoin_reorg_depth",
                "Depth of chain reorganizations",
                vec![1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0],
            ),
        }
    }

    /// Format all metrics in Prometheus exposition format.
    pub fn format_prometheus(&self) -> String {
        let mut output = String::with_capacity(4096);

        // Block metrics
        output.push_str(&self.blocks_processed.format_prometheus());
        output.push_str(&self.block_height.format_prometheus());
        output.push_str(&self.block_validation_seconds.format_prometheus());
        output.push_str(&self.block_connect_seconds.format_prometheus());
        output.push_str(&self.block_size_bytes.format_prometheus());

        // P2P metrics
        output.push_str(&self.peers_connected.format_prometheus());
        output.push_str(&self.peers_inbound.format_prometheus());
        output.push_str(&self.peers_outbound.format_prometheus());
        output.push_str(&self.messages_received.format_prometheus());
        output.push_str(&self.messages_sent.format_prometheus());
        output.push_str(&self.bytes_received.format_prometheus());
        output.push_str(&self.bytes_sent.format_prometheus());
        output.push_str(&self.peer_disconnections.format_prometheus());

        // Mempool metrics
        output.push_str(&self.mempool_size.format_prometheus());
        output.push_str(&self.mempool_bytes.format_prometheus());
        output.push_str(&self.mempool_accepted.format_prometheus());
        output.push_str(&self.mempool_rejected.format_prometheus());

        // UTXO cache metrics
        output.push_str(&self.utxo_cache_size.format_prometheus());
        output.push_str(&self.utxo_cache_hits.format_prometheus());
        output.push_str(&self.utxo_cache_misses.format_prometheus());
        output.push_str(&self.utxo_flush_seconds.format_prometheus());

        // RPC metrics
        output.push_str(&self.rpc_requests.format_prometheus());
        output.push_str(&self.rpc_errors.format_prometheus());
        output.push_str(&self.rpc_latency_seconds.format_prometheus());

        // Script verification metrics
        output.push_str(&self.script_verifications.format_prometheus());
        output.push_str(&self.script_verification_seconds.format_prometheus());

        // Reorg metrics
        output.push_str(&self.chain_reorgs.format_prometheus());
        output.push_str(&self.reorg_depth.format_prometheus());

        output
    }
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Global metrics instance for easy access.
static METRICS: std::sync::OnceLock<NodeMetrics> = std::sync::OnceLock::new();

/// Initialize the global metrics instance.
pub fn init_metrics() -> &'static NodeMetrics {
    METRICS.get_or_init(NodeMetrics::new)
}

/// Get a reference to the global metrics.
pub fn metrics() -> &'static NodeMetrics {
    METRICS.get_or_init(NodeMetrics::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_counter() {
        let counter = Counter::new("test_counter", "A test counter");
        assert_eq!(counter.get(), 0);
        counter.inc();
        assert_eq!(counter.get(), 1);
        counter.inc_by(5);
        assert_eq!(counter.get(), 6);
    }

    #[test]
    fn test_gauge() {
        let gauge = Gauge::new("test_gauge", "A test gauge");
        assert_eq!(gauge.get(), 0);
        gauge.set(42);
        assert_eq!(gauge.get(), 42);
        gauge.inc();
        assert_eq!(gauge.get(), 43);
        gauge.dec();
        assert_eq!(gauge.get(), 42);
    }

    #[test]
    fn test_histogram() {
        let hist = Histogram::new("test_hist", "A test histogram", vec![1.0, 5.0, 10.0]);
        hist.observe(0.5);
        hist.observe(3.0);
        hist.observe(7.5);
        hist.observe(15.0);

        let output = hist.format_prometheus();
        assert!(output.contains("test_hist_bucket{le=\"1\"} 1"));
        assert!(output.contains("test_hist_bucket{le=\"5\"} 2"));
        assert!(output.contains("test_hist_bucket{le=\"10\"} 3"));
        assert!(output.contains("test_hist_bucket{le=\"+Inf\"} 4"));
        assert!(output.contains("test_hist_count 4"));
    }

    #[test]
    fn test_histogram_timer() {
        let hist = Histogram::with_default_buckets("timer_test", "Timer test");
        {
            let _timer = hist.start_timer();
            // Simulate some work
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // Timer should have observed something > 0
        let output = hist.format_prometheus();
        assert!(output.contains("timer_test_count 1"));
    }

    #[test]
    fn test_counter_prometheus_format() {
        let counter = Counter::new("my_counter", "Help text");
        counter.inc_by(42);
        let output = counter.format_prometheus();
        assert!(output.contains("# HELP my_counter Help text"));
        assert!(output.contains("# TYPE my_counter counter"));
        assert!(output.contains("my_counter 42"));
    }

    #[test]
    fn test_gauge_prometheus_format() {
        let gauge = Gauge::new("my_gauge", "Gauge help");
        gauge.set(-10);
        let output = gauge.format_prometheus();
        assert!(output.contains("# TYPE my_gauge gauge"));
        assert!(output.contains("my_gauge -10"));
    }

    #[test]
    fn test_node_metrics() {
        let metrics = NodeMetrics::new();
        metrics.blocks_processed.inc();
        metrics.block_height.set(100);
        metrics.peers_connected.set(8);
        metrics.mempool_size.set(1500);

        let output = metrics.format_prometheus();
        assert!(output.contains("qubitcoin_blocks_processed_total 1"));
        assert!(output.contains("qubitcoin_block_height 100"));
        assert!(output.contains("qubitcoin_peers_connected 8"));
        assert!(output.contains("qubitcoin_mempool_size 1500"));
    }

    #[test]
    fn test_global_metrics() {
        let m = metrics();
        m.blocks_processed.inc();
        assert!(m.blocks_processed.get() >= 1);
    }

    #[test]
    fn test_full_prometheus_output() {
        let metrics = NodeMetrics::new();
        metrics.blocks_processed.inc_by(100);
        metrics.block_height.set(800000);
        metrics.peers_connected.set(125);
        metrics.mempool_size.set(50000);
        metrics.rpc_requests.inc_by(10000);
        metrics.bytes_received.inc_by(1_000_000_000);

        let output = metrics.format_prometheus();
        // Should contain all metric types
        assert!(output.contains("# TYPE qubitcoin_blocks_processed_total counter"));
        assert!(output.contains("# TYPE qubitcoin_block_height gauge"));
        assert!(output.contains("# TYPE qubitcoin_block_validation_seconds histogram"));
        assert!(output.len() > 1000); // Should be substantial
    }

    #[test]
    fn test_concurrent_counter() {
        let counter = Arc::new(Counter::new("concurrent", "Test"));
        let mut handles = vec![];

        for _ in 0..10 {
            let c = counter.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    c.inc();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.get(), 10000);
    }

    #[test]
    fn test_concurrent_gauge() {
        let gauge = Arc::new(Gauge::new("concurrent_gauge", "Test"));
        let mut handles = vec![];

        for _ in 0..10 {
            let g = gauge.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    g.inc();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(gauge.get(), 1000);
    }
}
