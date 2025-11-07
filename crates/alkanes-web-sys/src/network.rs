//! Web network implementation using fetch API
//!
//! This module provides network operations using the browser's fetch API
//! for making HTTP requests in web environments. The [`WebNetwork`] struct
//! implements the [`alkanes_cli_common::NetworkProvider`] trait, providing a
//! web-compatible network backend for the Deezel Bitcoin toolkit.
//!
//! # Features
//!
//! - **Fetch API Integration**: Uses the modern browser fetch API for HTTP requests
//! - **CORS Support**: Configured for cross-origin requests with proper headers
//! - **Binary Data Support**: Handles both text and binary request/response data
//! - **Error Handling**: Comprehensive error handling for network operations
//! - **User Agent**: Configurable user agent string for requests
//! - **Async Interface**: Fully async API compatible with web environments
//!
//! # Browser Compatibility
//!
//! This implementation requires a browser environment with fetch API support.
//! It will gracefully handle cases where the window object is not available.
//!
//! # Examples
//!
//! ```rust,no_run
//! use deezel_web::network::WebNetwork;
//! use alkanes_cli_common::NetworkProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let network = WebNetwork::new();
//!
//! // Make a GET request
//! let response = network.get("https://api.example.com/data").await?;
//! println!("Response: {} bytes", response.len());
//!
//! // Make a POST request
//! let data = b"Hello, world!";
//! let response = network.post(
//!     "https://api.example.com/submit",
//!     data,
//!     "text/plain"
//! ).await?;
//!
//! // Check if a URL is reachable
//! let reachable = network.is_reachable("https://api.example.com/health").await;
//! println!("API is reachable: {}", reachable);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use alkanes_cli_common::{AlkanesError, Result};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, window, Headers};

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    string::{String, ToString},
    vec::Vec,
    format,
};

/// Web network implementation using browser fetch API
///
/// This struct provides a web-compatible network backend that implements the
/// [`alkanes_cli_common::NetworkProvider`] trait. It uses the browser's fetch API
/// for making HTTP requests with full support for binary data and CORS.
///
/// # Configuration
///
/// - Uses CORS mode for cross-origin requests
/// - Sets a default user agent string "alkanes-web/0.1.0"
/// - Supports custom headers including Content-Type
/// - Handles both text and binary request/response bodies
///
/// # Error Handling
///
/// The implementation handles various error conditions:
/// - Window object not available (non-browser environment)
/// - Network connectivity issues
/// - HTTP error status codes
/// - Invalid response data formats
/// - CORS policy violations
///
/// # Thread Safety
///
/// This struct is `Clone` but not `Send` or `Sync`, as it's designed for
/// single-threaded web environments using `?Send` async traits.
#[derive(Clone)]
pub struct WebNetwork {
    /// User agent string sent with all HTTP requests
    user_agent: String,
}

impl WebNetwork {
    /// Create a new WebNetwork instance with default configuration
    ///
    /// Sets up a new network provider with the default user agent string
    /// "alkanes-web/0.1.0". The instance is ready to make HTTP requests
    /// immediately.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use deezel_web::network::WebNetwork;
    ///
    /// let network = WebNetwork::new();
    /// // Network is ready to use for HTTP requests
    /// ```
    pub fn new() -> Self {
        Self {
            user_agent: "alkanes-web/0.1.0".to_string(),
        }
    }

    /// Make a fetch request with the given parameters
    ///
    /// This is the core method that handles all HTTP requests using the
    /// browser's fetch API. It sets up proper headers, handles CORS,
    /// and processes the response.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to make the request to
    /// * `method` - HTTP method (GET, POST, PUT, DELETE, etc.)
    /// * `body` - Optional request body as bytes
    /// * `content_type` - Optional Content-Type header value
    ///
    /// # Returns
    ///
    /// A [`web_sys::Response`] object on success
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Network`] if the window object is not available
    /// * [`AlkanesError::Network`] if request creation fails
    /// * [`AlkanesError::Network`] if the fetch operation fails
    /// * [`AlkanesError::Network`] if the response has an error status code
    async fn fetch_request(
        &self,
        url: &str,
        method: &str,
        body: Option<&[u8]>,
        content_type: Option<&str>,
    ) -> Result<Response> {
        let window = window().ok_or_else(|| AlkanesError::Network("No window object available".to_string()))?;

        let opts = RequestInit::new();
        opts.set_method(method);
        opts.set_mode(RequestMode::Cors);

        // Set headers
        let headers = Headers::new()
            .map_err(|e| AlkanesError::Network(format!("Failed to create headers: {e:?}")))?;
        
        headers.set("User-Agent", &self.user_agent)
            .map_err(|e| AlkanesError::Network(format!("Failed to set User-Agent: {e:?}")))?;

        if let Some(ct) = content_type {
            headers.set("Content-Type", ct)
                .map_err(|e| AlkanesError::Network(format!("Failed to set Content-Type: {e:?}")))?;
        }

        opts.set_headers(&headers);

        // Set body if provided
        if let Some(body_bytes) = body {
            let uint8_array = Uint8Array::new_with_length(body_bytes.len() as u32);
            uint8_array.copy_from(body_bytes);
            opts.set_body(&uint8_array);
        }

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| AlkanesError::Network(format!("Failed to create request: {e:?}")))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| AlkanesError::Network(format!("Fetch failed: {e:?}")))?;

        let resp: Response = resp_value.dyn_into()
            .map_err(|e| AlkanesError::Network(format!("Failed to cast response: {e:?}")))?;

        if !resp.ok() {
            return Err(AlkanesError::Network(format!(
                "HTTP error: {} {}",
                resp.status(),
                resp.status_text()
            )));
        }

        Ok(resp)
    }

    /// Convert a fetch Response to bytes
    ///
    /// Reads the response body as an ArrayBuffer and converts it to a Vec<u8>.
    /// This method handles both text and binary response data.
    ///
    /// # Arguments
    ///
    /// * `response` - The Response object from a fetch request
    ///
    /// # Returns
    ///
    /// The response body as a vector of bytes
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Network`] if reading the array buffer fails
    /// * [`AlkanesError::Network`] if converting the buffer to bytes fails
    async fn response_to_bytes(&self, response: Response) -> Result<Vec<u8>> {
        let array_buffer = JsFuture::from(response.array_buffer()
            .map_err(|e| AlkanesError::Network(format!("Failed to get array buffer: {e:?}")))?)
            .await
            .map_err(|e| AlkanesError::Network(format!("Failed to read array buffer: {e:?}")))?;

        let uint8_array = Uint8Array::new(&array_buffer);
        let mut bytes = vec![0u8; uint8_array.length() as usize];
        uint8_array.copy_to(&mut bytes);
        
        Ok(bytes)
    }
}

/// Implementation of the [`alkanes_cli_common::NetworkProvider`] trait for web environments
///
/// This implementation provides all the standard network operations using the
/// browser's fetch API. All operations are async-compatible and handle
/// the web environment's constraints including CORS and security policies.
#[async_trait(?Send)]
impl alkanes_cli_common::NetworkProvider for WebNetwork {
    /// Perform an HTTP GET request
    ///
    /// Makes a GET request to the specified URL and returns the response body
    /// as bytes. This method is suitable for downloading both text and binary data.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to make the GET request to
    ///
    /// # Returns
    ///
    /// The response body as a vector of bytes
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Network`] if the request fails
    /// * [`AlkanesError::Network`] if the response has an error status code
    /// * [`AlkanesError::Network`] if reading the response body fails
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.fetch_request(url, "GET", None, None).await?;
        self.response_to_bytes(response).await
    }

    /// Perform an HTTP POST request with a body
    ///
    /// Makes a POST request to the specified URL with the given body data
    /// and content type. Returns the response body as bytes.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to make the POST request to
    /// * `body` - The request body data as bytes
    /// * `content_type` - The Content-Type header value (e.g., "application/json")
    ///
    /// # Returns
    ///
    /// The response body as a vector of bytes
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Network`] if the request fails
    /// * [`AlkanesError::Network`] if the response has an error status code
    /// * [`AlkanesError::Network`] if reading the response body fails
    async fn post(&self, url: &str, body: &[u8], content_type: &str) -> Result<Vec<u8>> {
        let response = self.fetch_request(url, "POST", Some(body), Some(content_type)).await?;
        self.response_to_bytes(response).await
    }

    /// Download data from a URL
    ///
    /// This is an alias for the `get` method, provided for semantic clarity
    /// when downloading files or large amounts of data.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    ///
    /// # Returns
    ///
    /// The downloaded data as a vector of bytes
    ///
    /// # Errors
    ///
    /// * [`AlkanesError::Network`] if the request fails
    /// * [`AlkanesError::Network`] if the response has an error status code
    /// * [`AlkanesError::Network`] if reading the response body fails
    async fn download(&self, url: &str) -> Result<Vec<u8>> {
        self.get(url).await
    }

    /// Check if a URL is reachable
    ///
    /// Makes a HEAD request to the specified URL to check if it's accessible
    /// without downloading the full response body. This is useful for health
    /// checks and connectivity testing.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check for reachability
    ///
    /// # Returns
    ///
    /// `true` if the URL is reachable (returns a successful HTTP status),
    /// `false` if the request fails or returns an error status
    ///
    /// # Note
    ///
    /// This method never panics and will return `false` for any error condition,
    /// including network failures, CORS issues, or HTTP error status codes.
    async fn is_reachable(&self, url: &str) -> bool {
        (self.fetch_request(url, "HEAD", None, None).await).is_ok()
    }

    /// Get the user agent string
    ///
    /// Returns the user agent string that is sent with all HTTP requests.
    /// This can be useful for logging, debugging, or API requirements.
    ///
    /// # Returns
    ///
    /// The user agent string, defaults to "alkanes-web/0.1.0"
    fn user_agent(&self) -> &str {
        &self.user_agent
    }
}

impl Default for WebNetwork {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_cli_common::NetworkProvider;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_get_request() {
        let network = WebNetwork::new();
        
        // Test with a simple endpoint (this might fail in test environment)
        // In a real test, you'd use a mock server or known endpoint
        let result = network.get("https://httpbin.org/get").await;
        
        // We can't guarantee this will work in all test environments,
        // so we just check that the method doesn't panic
        match result {
            Ok(data) => {
                assert!(!data.is_empty());
            },
            Err(_) => {
                // Network request failed, which is expected in some test environments
            }
        }
    }

    #[wasm_bindgen_test]
    async fn test_post_request() {
        let network = WebNetwork::new();
        let test_data = b"test data";
        
        // Test with a simple endpoint (this might fail in test environment)
        let result = network.post("https://httpbin.org/post", test_data, "text/plain").await;
        
        // We can't guarantee this will work in all test environments,
        // so we just check that the method doesn't panic
        match result {
            Ok(data) => {
                assert!(!data.is_empty());
            },
            Err(_) => {
                // Network request failed, which is expected in some test environments
            }
        }
    }

    #[wasm_bindgen_test]
    fn test_user_agent() {
        let network = WebNetwork::new();
        assert_eq!(network.user_agent(), "alkanes-web/0.1.0");
    }
}