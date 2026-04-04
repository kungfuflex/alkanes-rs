//! Logging initialization and log categories.
//!
//! Maps to: `src/logging.h` and `src/logging.cpp` in Bitcoin Core.
//!
//! Provides log-level abstraction and log categories matching Bitcoin Core's
//! `-debug=` categories for selective subsystem logging.
//!
//! Prefer `init_tracing` over the deprecated `init_logging` for new code.
//! `tracing` gives structured fields, spans, and better observability compared
//! to Bitcoin Core's `LogPrintf`.

use std::sync::Once;

static INIT: Once = Once::new();

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    /// Critical errors that may cause data loss or shutdown.
    Error,
    /// Conditions that are not errors but may require attention.
    Warn,
    /// General operational information (default level).
    Info,
    /// Detailed information useful for diagnosing problems.
    Debug,
    /// Very fine-grained diagnostic output.
    Trace,
}

impl LogLevel {
    /// Parse a log level from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "error" => Some(LogLevel::Error),
            "warn" | "warning" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }

    /// Convert to the `log` crate's `LevelFilter`.
    #[deprecated(note = "use to_tracing_level() and init_tracing() instead")]
    pub fn to_log_filter(&self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }

    /// Convert to a `tracing` level filter string suitable for env-filter.
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }
}

/// Log categories matching Bitcoin Core's `-debug=` categories.
///
/// Used to selectively enable verbose logging for specific subsystems.
/// Maps to: `BCLog::LogFlags` in Bitcoin Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogCategory {
    /// Enable all categories.
    All,
    /// Network messages and connections.
    Net,
    /// Tor connection handling.
    Tor,
    /// Mempool operations.
    Mempool,
    /// HTTP server.
    Http,
    /// Benchmarking.
    Bench,
    /// ZMQ notifications.
    Zmq,
    /// Wallet database operations.
    Walletdb,
    /// RPC calls.
    Rpc,
    /// Fee estimation.
    Estimatefee,
    /// Address manager.
    Addrman,
    /// Coin selection.
    Selectcoins,
    /// Reindexing.
    Reindex,
    /// Compact block relay.
    Cmpctblock,
    /// Block pruning.
    Prune,
    /// SOCKS5 proxy.
    Proxy,
    /// Mempool rejection.
    Mempoolrej,
    /// Libevent.
    Libevent,
    /// Coin database.
    Coindb,
    /// LevelDB.
    Leveldb,
    /// Block validation.
    Validation,
}

impl LogCategory {
    /// Parse a category from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "all" | "1" => Some(LogCategory::All),
            "net" => Some(LogCategory::Net),
            "tor" => Some(LogCategory::Tor),
            "mempool" => Some(LogCategory::Mempool),
            "http" => Some(LogCategory::Http),
            "bench" => Some(LogCategory::Bench),
            "zmq" => Some(LogCategory::Zmq),
            "walletdb" | "db" => Some(LogCategory::Walletdb),
            "rpc" => Some(LogCategory::Rpc),
            "estimatefee" => Some(LogCategory::Estimatefee),
            "addrman" => Some(LogCategory::Addrman),
            "selectcoins" => Some(LogCategory::Selectcoins),
            "reindex" => Some(LogCategory::Reindex),
            "cmpctblock" => Some(LogCategory::Cmpctblock),
            "prune" => Some(LogCategory::Prune),
            "proxy" => Some(LogCategory::Proxy),
            "mempoolrej" => Some(LogCategory::Mempoolrej),
            "libevent" => Some(LogCategory::Libevent),
            "coindb" => Some(LogCategory::Coindb),
            "leveldb" => Some(LogCategory::Leveldb),
            "validation" => Some(LogCategory::Validation),
            _ => None,
        }
    }

    /// Return the canonical string name for this category.
    pub fn as_str(&self) -> &str {
        match self {
            LogCategory::All => "all",
            LogCategory::Net => "net",
            LogCategory::Tor => "tor",
            LogCategory::Mempool => "mempool",
            LogCategory::Http => "http",
            LogCategory::Bench => "bench",
            LogCategory::Zmq => "zmq",
            LogCategory::Walletdb => "walletdb",
            LogCategory::Rpc => "rpc",
            LogCategory::Estimatefee => "estimatefee",
            LogCategory::Addrman => "addrman",
            LogCategory::Selectcoins => "selectcoins",
            LogCategory::Reindex => "reindex",
            LogCategory::Cmpctblock => "cmpctblock",
            LogCategory::Prune => "prune",
            LogCategory::Proxy => "proxy",
            LogCategory::Mempoolrej => "mempoolrej",
            LogCategory::Libevent => "libevent",
            LogCategory::Coindb => "coindb",
            LogCategory::Leveldb => "leveldb",
            LogCategory::Validation => "validation",
        }
    }
}

/// Initialize logging with the given level using the `tracing` + `tracing-subscriber` stack.
///
/// This should be called once at application startup. Subsequent calls are
/// silently ignored (idempotent via `std::sync::Once`).
///
/// Sets up a `tracing-subscriber` with:
/// - An env-filter that respects the `RUST_LOG` environment variable (falling
///   back to the provided `level` when `RUST_LOG` is not set).
/// - Timestamps with millisecond precision.
/// - Module paths in each log line.
///
/// This is the preferred initialization path. See also the deprecated
/// `init_logging` for backward compatibility.
#[cfg(feature = "native")]
pub fn init_tracing(level: LogLevel) {
    use tracing_subscriber::EnvFilter;

    INIT.call_once(|| {
        let default_directive = level.to_tracing_level().to_string();
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&default_directive));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_timer(tracing_subscriber::fmt::time::SystemTime)
            .init();
    });
}

#[cfg(not(feature = "native"))]
pub fn init_tracing(_level: LogLevel) {
    // tracing_subscriber not available on wasm; no-op
}

/// Initialize logging with the given level.
///
/// **Deprecated**: use `init_tracing` instead for structured logging,
/// spans, and better observability.
///
/// This should be called once at application startup. Subsequent calls are
/// silently ignored (idempotent via `std::sync::Once`).
#[deprecated(note = "use init_tracing() for structured logging via the tracing crate")]
pub fn init_logging(level: LogLevel) {
    // Delegate to init_tracing so callers still get modern behavior.
    init_tracing(level);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- LogLevel tests ---

    #[test]
    fn test_log_level_from_str_basic() {
        assert_eq!(LogLevel::from_str("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("trace"), Some(LogLevel::Trace));
    }

    #[test]
    fn test_log_level_from_str_case_insensitive() {
        assert_eq!(LogLevel::from_str("ERROR"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("Info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("Warning"), Some(LogLevel::Warn));
    }

    #[test]
    fn test_log_level_from_str_invalid() {
        assert_eq!(LogLevel::from_str(""), None);
        assert_eq!(LogLevel::from_str("verbose"), None);
        assert_eq!(LogLevel::from_str("fatal"), None);
    }

    #[test]
    fn test_log_level_to_tracing_level() {
        assert_eq!(LogLevel::Error.to_tracing_level(), tracing::Level::ERROR);
        assert_eq!(LogLevel::Warn.to_tracing_level(), tracing::Level::WARN);
        assert_eq!(LogLevel::Info.to_tracing_level(), tracing::Level::INFO);
        assert_eq!(LogLevel::Debug.to_tracing_level(), tracing::Level::DEBUG);
        assert_eq!(LogLevel::Trace.to_tracing_level(), tracing::Level::TRACE);
    }

    #[test]
    #[allow(deprecated)]
    fn test_log_level_to_filter_deprecated() {
        // Ensure the deprecated method still works for backward compat.
        assert_eq!(LogLevel::Error.to_log_filter(), log::LevelFilter::Error);
        assert_eq!(LogLevel::Info.to_log_filter(), log::LevelFilter::Info);
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
    }

    // --- LogCategory tests ---

    #[test]
    fn test_log_category_from_str_all() {
        assert_eq!(LogCategory::from_str("all"), Some(LogCategory::All));
        assert_eq!(LogCategory::from_str("1"), Some(LogCategory::All));
        assert_eq!(LogCategory::from_str("ALL"), Some(LogCategory::All));
    }

    #[test]
    fn test_log_category_from_str_roundtrip() {
        let categories = [
            LogCategory::All,
            LogCategory::Net,
            LogCategory::Tor,
            LogCategory::Mempool,
            LogCategory::Http,
            LogCategory::Bench,
            LogCategory::Zmq,
            LogCategory::Walletdb,
            LogCategory::Rpc,
            LogCategory::Estimatefee,
            LogCategory::Addrman,
            LogCategory::Selectcoins,
            LogCategory::Reindex,
            LogCategory::Cmpctblock,
            LogCategory::Prune,
            LogCategory::Proxy,
            LogCategory::Mempoolrej,
            LogCategory::Libevent,
            LogCategory::Coindb,
            LogCategory::Leveldb,
            LogCategory::Validation,
        ];

        for cat in &categories {
            let s = cat.as_str();
            let parsed = LogCategory::from_str(s)
                .unwrap_or_else(|| panic!("Failed to parse category: {}", s));
            assert_eq!(*cat, parsed, "Roundtrip failed for {}", s);
        }
    }

    #[test]
    fn test_log_category_from_str_invalid() {
        assert_eq!(LogCategory::from_str(""), None);
        assert_eq!(LogCategory::from_str("unknown"), None);
        assert_eq!(LogCategory::from_str("wallet"), None);
    }

    #[test]
    fn test_log_category_db_alias() {
        // "db" should parse to Walletdb (matching Bitcoin Core behavior)
        assert_eq!(LogCategory::from_str("db"), Some(LogCategory::Walletdb));
    }

    #[test]
    fn test_log_category_as_str() {
        assert_eq!(LogCategory::Net.as_str(), "net");
        assert_eq!(LogCategory::Mempool.as_str(), "mempool");
        assert_eq!(LogCategory::Validation.as_str(), "validation");
    }
}
