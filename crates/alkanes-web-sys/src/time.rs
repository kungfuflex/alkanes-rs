//! Web time implementation using Performance API
//!
//! This module provides time operations using the browser's Performance API
//! and other web-compatible timing mechanisms. The [`WebTime`] struct
//! implements the [`alkanes_cli_common::TimeProvider`] trait, providing a
//! web-compatible time backend for the Deezel Bitcoin toolkit.
//!
//! # Features
//!
//! - **High-Resolution Timing**: Uses Performance API for precise measurements
//! - **Fallback Support**: Falls back to Date API when Performance API is unavailable
//! - **Async Sleep**: Implements sleep functionality using setTimeout
//! - **Unix Timestamps**: Provides both second and millisecond precision timestamps
//! - **Cross-Browser Compatibility**: Works across different browser environments
//!
//! # Browser Compatibility
//!
//! This implementation requires a browser environment. It will use the Performance API
//! when available for high-resolution timing, falling back to the Date API otherwise.
//!
//! # Examples
//!
//! ```rust,no_run
//! use deezel_web::time::WebTime;
//! use alkanes_cli_common::TimeProvider;
//!
//! # async fn example() {
//! let time = WebTime::new();
//!
//! // Get current time in seconds since Unix epoch
//! let now_secs = time.now_secs();
//! println!("Current time: {} seconds", now_secs);
//!
//! // Get current time in milliseconds since Unix epoch
//! let now_millis = time.now_millis();
//! println!("Current time: {} milliseconds", now_millis);
//!
//! // Sleep for 1 second
//! time.sleep_ms(1000).await;
//! println!("Slept for 1 second");
//! # }
//! ```

use crate::platform;
use async_trait::async_trait;

#[cfg(target_arch = "wasm32")]
extern crate alloc;

/// Web time implementation using platform abstractions
///
/// Works in both browser and Node.js environments
#[derive(Clone)]
pub struct WebTime;

impl WebTime {
    /// Create a new WebTime instance
    pub fn new() -> Self {
        Self
    }

    /// Get time from Date API (works in both browser and Node.js)
    fn get_date_now(&self) -> f64 {
        js_sys::Date::now()
    }

    /// Get high-resolution time from Performance API if available (browser only)
    #[allow(dead_code)]
    pub fn get_performance_now(&self) -> Option<f64> {
        if platform::is_browser() {
            web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
        } else {
            None
        }
    }
}

#[async_trait(?Send)]
impl alkanes_cli_common::TimeProvider for WebTime {
    fn now_secs(&self) -> u64 {
        platform::get_timestamp_secs()
    }

    fn now_millis(&self) -> u64 {
        platform::get_timestamp_ms()
    }

    async fn sleep_ms(&self, ms: u64) {
        let _ = platform::sleep_ms(ms).await;
    }
}

impl Default for WebTime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_cli_common::TimeProvider;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_now_secs() {
        let time = WebTime::new();
        let secs = time.now_secs();
        
        // Should be a reasonable timestamp (after 2020)
        assert!(secs > 1577836800); // Jan 1, 2020
    }

    #[wasm_bindgen_test]
    fn test_now_millis() {
        let time = WebTime::new();
        let millis = time.now_millis();
        
        // Should be a reasonable timestamp (after 2020)
        assert!(millis > 1577836800000); // Jan 1, 2020 in milliseconds
        
        // Milliseconds should be larger than seconds
        let secs = time.now_secs();
        assert!(millis > secs * 1000);
    }

    #[wasm_bindgen_test]
    async fn test_sleep_ms() {
        let time = WebTime::new();
        let start = time.now_millis();
        
        // Sleep for 100ms
        time.sleep_ms(100).await;
        
        let end = time.now_millis();
        let elapsed = end - start;
        
        // Should have slept for at least 90ms (allowing for some variance)
        assert!(elapsed >= 90);
        
        // Should not have slept for more than 200ms (allowing for some variance)
        assert!(elapsed < 200);
    }

    #[wasm_bindgen_test]
    fn test_performance_api() {
        let time = WebTime::new();
        
        if let Some(perf_time) = time.get_performance_now() {
            // Performance.now() should return a positive number
            assert!(perf_time >= 0.0);
        }
        
        // Date.now() should always work
        let date_time = time.get_date_now();
        assert!(date_time > 0.0);
    }
}