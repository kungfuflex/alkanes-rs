//! qubitcoin-util: Utility functions for Qubitcoin.
//!
//! Maps to: various utility files in Bitcoin Core (logging, time, args).
//!
//! Provides:
//! - [`logging`]: Logging initialization and log categories matching Bitcoin Core's `-debug=` categories.
//! - [`time`]: Time utilities including a mockable clock for testing.
//! - [`args`]: Command-line argument parser matching Bitcoin Core's `-key=value` style.

/// Command-line argument parser matching Bitcoin Core's `-key=value` style.
pub mod args;
/// Logging initialization and log categories matching Bitcoin Core's `-debug=` categories.
#[cfg(not(target_arch = "wasm32"))]
pub mod logging;
/// Built-in Prometheus metrics for node observability.
pub mod metrics;
/// Time utilities including a mockable `Clock` trait for deterministic testing.
pub mod time;
