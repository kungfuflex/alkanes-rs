//! Web utilities for deezel-web
//!
//! This module provides utility functions and helpers specific to web environments,
//! including WASM interop, browser feature detection, and common web operations.

use deezel_common::{DeezelError, Result};
use js_sys::{Array, Object, Uint8Array};
use wasm_bindgen::prelude::*;
use web_sys::{window, Document, Location, Navigator, Window};

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
/// Web environment utilities
pub struct WebUtils;

impl WebUtils {
    /// Check if we're running in a web environment
    pub fn is_web_environment() -> bool {
        window().is_some()
    }

    /// Get the current window object
    pub fn get_window() -> Result<Window> {
        window().ok_or_else(|| DeezelError::Network("No window object available".to_string()))
    }

    /// Get the document object
    pub fn get_document() -> Result<Document> {
        let window = Self::get_window()?;
        window.document()
            .ok_or_else(|| DeezelError::Network("No document object available".to_string()))
    }

    /// Get the navigator object
    pub fn get_navigator() -> Result<Navigator> {
        let window = Self::get_window()?;
        Ok(window.navigator())
    }

    /// Get the location object
    pub fn get_location() -> Result<Location> {
        let window = Self::get_window()?;
        Ok(window.location())
    }

    /// Get the current URL
    pub fn get_current_url() -> Result<String> {
        let location = Self::get_location()?;
        location.href()
            .map_err(|e| DeezelError::Network(format!("Failed to get current URL: {e:?}")))
    }

    /// Get the user agent string
    pub fn get_user_agent() -> Result<String> {
        let navigator = Self::get_navigator()?;
        Ok(navigator.user_agent()
            .unwrap_or_else(|_| "Unknown".to_string()))
    }

    /// Check if localStorage is available
    pub fn is_local_storage_available() -> bool {
        if let Ok(window) = Self::get_window() {
            window.local_storage().is_ok()
        } else {
            false
        }
    }

    /// Check if Web Crypto API is available
    pub fn is_web_crypto_available() -> bool {
        if let Ok(window) = Self::get_window() {
            window.crypto().is_ok()
        } else {
            false
        }
    }

    /// Check if fetch API is available
    pub fn is_fetch_available() -> bool {
        if let Ok(window) = Self::get_window() {
            js_sys::Reflect::has(&window, &"fetch".into()).unwrap_or(false)
        } else {
            false
        }
    }

    /// Convert JavaScript Uint8Array to Rust `Vec<u8>`
    pub fn uint8_array_to_vec(array: &Uint8Array) -> Vec<u8> {
        let mut vec = vec![0u8; array.length() as usize];
        array.copy_to(&mut vec);
        vec
    }

    /// Convert Rust `Vec<u8>` to JavaScript Uint8Array
    pub fn vec_to_uint8_array(vec: &[u8]) -> Uint8Array {
        let array = Uint8Array::new_with_length(vec.len() as u32);
        array.copy_from(vec);
        array
    }

    /// Convert JavaScript Array to Rust `Vec<String>`
    pub fn js_array_to_string_vec(array: &Array) -> Vec<String> {
        let mut vec = Vec::new();
        for i in 0..array.length() {
            if let Ok(value) = array.get(i).dyn_into::<js_sys::JsString>() {
                vec.push(String::from(value));
            }
        }
        vec
    }

    /// Convert Rust `Vec<String>` to JavaScript Array
    pub fn string_vec_to_js_array(vec: &[String]) -> Array {
        let array = Array::new();
        for item in vec {
            array.push(&JsValue::from_str(item));
        }
        array
    }

    /// Create a JavaScript object from key-value pairs
    pub fn create_js_object(pairs: &[(&str, &JsValue)]) -> Object {
        let obj = Object::new();
        for (key, value) in pairs {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(key), value);
        }
        obj
    }

    /// Get a value from a JavaScript object
    pub fn get_js_object_value(obj: &Object, key: &str) -> Option<JsValue> {
        js_sys::Reflect::get(obj, &JsValue::from_str(key)).ok()
    }

    /// Set a value in a JavaScript object
    pub fn set_js_object_value(obj: &Object, key: &str, value: &JsValue) -> Result<()> {
        js_sys::Reflect::set(obj, &JsValue::from_str(key), value)
            .map_err(|e| DeezelError::Serialization(format!("Failed to set object value: {e:?}")))?;
        Ok(())
    }

    /// Check if running in a secure context (HTTPS)
    pub fn is_secure_context() -> bool {
        if let Ok(window) = Self::get_window() {
            window.is_secure_context()
        } else {
            false
        }
    }

    /// Get the origin of the current page
    pub fn get_origin() -> Result<String> {
        let location = Self::get_location()?;
        location.origin()
            .map_err(|e| DeezelError::Network(format!("Failed to get origin: {e:?}")))
    }

    /// Check if running in an iframe
    pub fn is_in_iframe() -> bool {
        if let Ok(window) = Self::get_window() {
            if let Ok(parent) = js_sys::Reflect::get(&window, &"parent".into()) {
                !parent.loose_eq(&window.into())
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get browser capabilities summary
    pub fn get_browser_capabilities() -> BrowserCapabilities {
        BrowserCapabilities {
            local_storage: Self::is_local_storage_available(),
            web_crypto: Self::is_web_crypto_available(),
            fetch: Self::is_fetch_available(),
            secure_context: Self::is_secure_context(),
            in_iframe: Self::is_in_iframe(),
            user_agent: Self::get_user_agent().unwrap_or_else(|_| "Unknown".to_string()),
        }
    }
}

/// Browser capabilities information
#[derive(Debug, Clone)]
pub struct BrowserCapabilities {
    pub local_storage: bool,
    pub web_crypto: bool,
    pub fetch: bool,
    pub secure_context: bool,
    pub in_iframe: bool,
    pub user_agent: String,
}

impl BrowserCapabilities {
    /// Check if all required capabilities are available
    pub fn has_required_capabilities(&self) -> bool {
        self.local_storage && self.web_crypto && self.fetch
    }

    /// Get a list of missing capabilities
    pub fn missing_capabilities(&self) -> Vec<String> {
        let mut missing = Vec::new();
        
        if !self.local_storage {
            missing.push("localStorage".to_string());
        }
        if !self.web_crypto {
            missing.push("Web Crypto API".to_string());
        }
        if !self.fetch {
            missing.push("Fetch API".to_string());
        }
        
        missing
    }
}

/// Error handling utilities for web environments
pub mod error_utils {
    use super::*;

    /// Convert a JavaScript error to a DeezelError
    pub fn js_error_to_deezel_error(js_error: JsValue) -> DeezelError {
        let error_string = if js_error.is_string() {
            js_error.as_string().unwrap_or_else(|| "Unknown error".to_string())
        } else if let Ok(error_obj) = js_error.clone().dyn_into::<js_sys::Error>() {
            error_obj.message().as_string().unwrap_or_else(|| "Unknown error".to_string())
        } else {
            format!("JavaScript error: {js_error:?}")
        };
        
        DeezelError::Network(error_string)
    }

    /// Handle and log JavaScript errors
    pub fn handle_js_error(js_error: JsValue, context: &str) -> DeezelError {
        let error = js_error_to_deezel_error(js_error);
        crate::logging::console_log::error(&format!("Error in {context}: {error}"));
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_web_environment_detection() {
        assert!(WebUtils::is_web_environment());
    }

    #[wasm_bindgen_test]
    fn test_window_access() {
        let window = WebUtils::get_window();
        assert!(window.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_document_access() {
        let document = WebUtils::get_document();
        assert!(document.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_current_url() {
        let url = WebUtils::get_current_url();
        assert!(url.is_ok());
        
        let url_string = url.unwrap();
        assert!(!url_string.is_empty());
    }

    #[wasm_bindgen_test]
    fn test_user_agent() {
        let user_agent = WebUtils::get_user_agent();
        assert!(user_agent.is_ok());
        
        let ua_string = user_agent.unwrap();
        assert!(!ua_string.is_empty());
    }

    #[wasm_bindgen_test]
    fn test_capability_checks() {
        // These should generally be true in modern browsers
        assert!(WebUtils::is_local_storage_available());
        assert!(WebUtils::is_fetch_available());
        
        // Web Crypto might not be available in all test environments
        let _crypto_available = WebUtils::is_web_crypto_available();
    }

    #[wasm_bindgen_test]
    fn test_array_conversions() {
        let rust_vec = vec![1u8, 2, 3, 4, 5];
        let js_array = WebUtils::vec_to_uint8_array(&rust_vec);
        let converted_back = WebUtils::uint8_array_to_vec(&js_array);
        
        assert_eq!(rust_vec, converted_back);
    }

    #[wasm_bindgen_test]
    fn test_string_array_conversions() {
        let rust_vec = vec!["hello".to_string(), "world".to_string()];
        let js_array = WebUtils::string_vec_to_js_array(&rust_vec);
        let converted_back = WebUtils::js_array_to_string_vec(&js_array);
        
        assert_eq!(rust_vec, converted_back);
    }

    #[wasm_bindgen_test]
    fn test_js_object_operations() {
        let obj = WebUtils::create_js_object(&[
            ("key1", &JsValue::from_str("value1")),
            ("key2", &JsValue::from_f64(42.0)),
        ]);
        
        let value1 = WebUtils::get_js_object_value(&obj, "key1");
        assert!(value1.is_some());
        assert_eq!(value1.unwrap().as_string().unwrap(), "value1");
        
        let value2 = WebUtils::get_js_object_value(&obj, "key2");
        assert!(value2.is_some());
        assert_eq!(value2.unwrap().as_f64().unwrap(), 42.0);
    }

    #[wasm_bindgen_test]
    fn test_browser_capabilities() {
        let capabilities = WebUtils::get_browser_capabilities();
        
        // Should have basic capabilities in a modern browser
        assert!(capabilities.local_storage);
        assert!(capabilities.fetch);
        assert!(!capabilities.user_agent.is_empty());
        
        // Check if required capabilities are available
        let has_required = capabilities.has_required_capabilities();
        if !has_required {
            let missing = capabilities.missing_capabilities();
            web_sys::console::log_1(&format!("Missing capabilities: {missing:?}").into());
        }
    }
}