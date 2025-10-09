//! This module has been removed to eliminate code duplication.
//! All SMT functionality now uses the generic, well-tested implementation
//! from metashrew-runtime::smt::SMTHelper to ensure consistency between
//! test and production environments.
//!
//! Use metashrew_runtime::smt::SMTHelper instead.

// Re-export constants that were used by the old implementation
// These constants are no longer needed since we fixed the snapshot tracking
// to use the correct constants directly from metashrew_runtime::smt

// Re-export the generic SMT implementation for backward compatibility
#[allow(unused_imports)]
pub use metashrew_runtime::smt::SMTHelper;
