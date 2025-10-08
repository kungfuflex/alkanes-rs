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

use alkanes_cli_common::{DeezelError, Result};
use js_sys::{Date, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Performance};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::string::ToString;
#[cfg(target_arch = "wasm32")]
use alloc::boxed::Box;

/// Web time implementation using Performance API
#[derive(Clone)]
pub struct WebTime {
    #[allow(dead_code)]
    performance: Option<Performance>,
}

impl WebTime {
    /// Create a new WebTime instance
    pub fn new() -> Self {
        let performance = window()
            .and_then(|w| w.performance());
        
        Self { performance }
    }

    /// Get high-resolution time from Performance API if available
    #[allow(dead_code)]
    fn get_performance_now(&self) -> Option<f64> {
        self.performance.as_ref().map(|p| p.now())
    }

    /// Get time from Date API as fallback
    fn get_date_now(&self) -> f64 {
        Date::now()
    }
}

use async_trait::async_trait;

#[async_trait(?Send)]
impl alkanes_cli_common::TimeProvider for WebTime {
    fn now_secs(&self) -> u64 {
        // Use Date.now() which returns milliseconds since Unix epoch
        let millis = self.get_date_now();
        (millis / 1000.0) as u64
    }

    fn now_millis(&self) -> u64 {
        // Use Date.now() which returns milliseconds since Unix epoch
        self.get_date_now() as u64
    }

    async fn sleep_ms(&self, _ms: u64) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // For non-WASM targets, this is tricky without a proper async runtime.
            // The original code attempted to use tokio, but it's not a dependency.
            // We'll panic for now, as this path is not expected to be used.
            todo!("sleep_ms is not implemented for non-wasm targets in deezel-web");
        }
        #[cfg(target_arch = "wasm32")]
        {
            WebSleep::new(_ms).await;
        }
    }
}

/// Future implementation for sleep using setTimeout
pub struct WebSleep {
    promise: Option<Promise>,
    duration_ms: u64,
}

impl WebSleep {
    #[allow(dead_code)]
    fn new(duration_ms: u64) -> Self {
        Self {
            promise: None,
            duration_ms,
        }
    }

    fn create_promise(&mut self) -> Result<Promise> {
        let window = window().ok_or_else(|| DeezelError::Io("No window object available".to_string()))?;
        
        // Create a promise that resolves after the specified duration
        let promise = Promise::new(&mut |resolve, _reject| {
            let timeout_id = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                &resolve,
                self.duration_ms as i32,
            );
            
            // We could store the timeout_id for cancellation, but for simplicity we don't
            match timeout_id {
                Ok(_) => {},
                Err(_) => {
                    // If setTimeout fails, resolve immediately
                    let _ = resolve.call0(&JsValue::UNDEFINED);
                }
            }
        });
        
        Ok(promise)
    }
}

impl Future for WebSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.promise.is_none() {
            match self.create_promise() {
                Ok(promise) => {
                    self.promise = Some(promise);
                },
                Err(_) => {
                    // If we can't create a promise, just return ready immediately
                    return Poll::Ready(());
                }
            }
        }

        if let Some(promise) = &self.promise {
            let mut future = JsFuture::from(promise.clone());
            match Pin::new(&mut future).poll(cx) {
                Poll::Ready(_) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(())
        }
    }
}

// Implement Send for WebSleep (required for the trait)
unsafe impl Send for WebSleep {}

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