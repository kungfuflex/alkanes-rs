//! Web logging implementation using console API
//!
//! This module provides logging operations using the browser's console API
//! for debugging and monitoring in web environments. The [`WebLogger`] struct
//! implements the [`alkanes_cli_common::LogProvider`] trait, providing a
//! web-compatible logging backend for the Deezel Bitcoin toolkit.
//!
//! # Features
//!
//! - **Console Integration**: Uses the browser's native console API
//! - **Multiple Log Levels**: Supports debug, info, warn, and error levels
//! - **Timestamp Formatting**: Automatically adds ISO timestamps to log messages
//! - **Debug Control**: Configurable debug message filtering
//! - **Convenience Macros**: Provides easy-to-use logging macros
//! - **Direct Console Access**: Utility functions for direct console logging
//!
//! # Browser Compatibility
//!
//! This implementation works in all modern browsers that support the console API.
//! Log messages will appear in the browser's developer console.
//!
//! # Examples
//!
//! ```rust,no_run
//! use deezel_web::logging::WebLogger;
//! use alkanes_cli_common::LogProvider;
//!
//! let logger = WebLogger::new();
//!
//! // Standard logging methods
//! logger.debug("Debug information");
//! logger.info("General information");
//! logger.warn("Warning message");
//! logger.error("Error occurred");
//!
//! // Check if debug is enabled
//! if logger.is_debug_enabled() {
//!     logger.debug("This will only log if debug is enabled");
//! }
//! ```
//!
//! # Convenience Macros
//!
//! ```rust,no_run
//! use deezel_web::{web_log, web_debug, web_info, web_warn, web_error};
//!
//! web_debug!("Debug message with formatting: {}", 42);
//! web_info!("Info: {}", "important information");
//! web_warn!("Warning: {:.2}", 3.14159);
//! web_error!("Error: {:?}", vec![1, 2, 3]);
//! ```
use alkanes_cli_common::LogProvider;
use web_sys::console;

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{format, string::String};
/// Web logging implementation using console API
#[derive(Clone)]
pub struct WebLogger {
    debug_enabled: bool,
}

impl WebLogger {
    /// Create a new WebLogger instance
    pub fn new() -> Self {
        Self {
            debug_enabled: true, // Enable debug by default in web environments
        }
    }

    /// Create a new WebLogger with debug setting
    pub fn with_debug(debug_enabled: bool) -> Self {
        Self { debug_enabled }
    }

    /// Format log message with timestamp and level
    fn format_message(&self, level: &str, message: &str) -> String {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        format!("[{timestamp}] [{level}] {message}")
    }
}

impl LogProvider for WebLogger {
    fn debug(&self, message: &str) {
        if self.debug_enabled {
            let formatted = self.format_message("DEBUG", message);
            console::debug_1(&formatted.into());
        }
    }

    fn info(&self, message: &str) {
        let formatted = self.format_message("INFO", message);
        console::info_1(&formatted.into());
    }

    fn warn(&self, message: &str) {
        let formatted = self.format_message("WARN", message);
        console::warn_1(&formatted.into());
    }

    fn error(&self, message: &str) {
        let formatted = self.format_message("ERROR", message);
        console::error_1(&formatted.into());
    }

    fn is_debug_enabled(&self) -> bool {
        self.debug_enabled
    }
}

impl Default for WebLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for direct console logging
pub mod console_log {
    use super::*;

    /// Log a debug message directly to console
    pub fn debug(message: &str) {
        console::debug_1(&message.into());
    }

    /// Log an info message directly to console
    pub fn info(message: &str) {
        console::info_1(&message.into());
    }

    /// Log a warning message directly to console
    pub fn warn(message: &str) {
        console::warn_1(&message.into());
    }

    /// Log an error message directly to console
    pub fn error(message: &str) {
        console::error_1(&message.into());
    }

    /// Log a general message to console
    pub fn log(message: &str) {
        console::log_1(&message.into());
    }
}

/// Macro for convenient logging in web environments
#[macro_export]
macro_rules! web_log {
    ($level:ident, $($arg:tt)*) => {
        $crate::logging::console_log::$level(&format!($($arg)*))
    };
}

/// Convenience macros for different log levels
#[macro_export]
macro_rules! web_debug {
    ($($arg:tt)*) => {
        web_log!(debug, $($arg)*)
    };
}

#[macro_export]
macro_rules! web_info {
    ($($arg:tt)*) => {
        web_log!(info, $($arg)*)
    };
}

#[macro_export]
macro_rules! web_warn {
    ($($arg:tt)*) => {
        web_log!(warn, $($arg)*)
    };
}

#[macro_export]
macro_rules! web_error {
    ($($arg:tt)*) => {
        web_log!(error, $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_logger_creation() {
        let logger = WebLogger::new();
        assert!(logger.is_debug_enabled());

        let logger_no_debug = WebLogger::with_debug(false);
        assert!(!logger_no_debug.is_debug_enabled());
    }

    #[wasm_bindgen_test]
    fn test_logging_methods() {
        let logger = WebLogger::new();
        
        // These will output to the browser console
        logger.debug("Test debug message");
        logger.info("Test info message");
        logger.warn("Test warning message");
        logger.error("Test error message");
        
        // Test that debug can be disabled
        let logger_no_debug = WebLogger::with_debug(false);
        logger_no_debug.debug("This debug message should not appear");
        logger_no_debug.info("This info message should appear");
    }

    #[wasm_bindgen_test]
    fn test_console_log_functions() {
        console_log::debug("Direct debug message");
        console_log::info("Direct info message");
        console_log::warn("Direct warning message");
        console_log::error("Direct error message");
        console_log::log("Direct log message");
    }

    #[wasm_bindgen_test]
    fn test_logging_macros() {
        web_debug!("Debug message with formatting: {}", 42);
        web_info!("Info message with formatting: {}", "test");
        web_warn!("Warning message with formatting: {:.2}", core::f32::consts::PI);
        web_error!("Error message with formatting: {:?}", vec![1, 2, 3]);
    }

    #[wasm_bindgen_test]
    fn test_message_formatting() {
        let logger = WebLogger::new();
        let formatted = logger.format_message("TEST", "test message");
        
        // Should contain the level and message
        assert!(formatted.contains("[TEST]"));
        assert!(formatted.contains("test message"));
        
        // Should contain a timestamp (ISO format)
        assert!(formatted.contains("T"));
        assert!(formatted.contains("Z"));
    }
}