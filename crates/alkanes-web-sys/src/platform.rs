//! Platform abstraction layer for WASM bindings
//!
//! This module provides functions that abstract platform-specific functionality,
//! allowing code to run in different environments (browser, Node.js, etc.)
//!
//! The key abstractions provided are:
//! - `fetch` - HTTP requests (works in both browser and Node.js)
//! - `console_log` - Console logging (works in both browser and Node.js)
//! - `is_browser` - Runtime environment detection
//! - `get_timestamp_ms` - Current time in milliseconds
//! - `sleep_ms` - Async sleep functionality
//! - `PlatformStorage` - Key-value storage abstraction

use alkanes_cli_common::{AlkanesError, Result};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use js_sys::{Object, Reflect};

/// Check if we're running in a browser environment
pub fn is_browser() -> bool {
    let global = js_sys::global();
    Reflect::has(&global, &"window".into()).unwrap_or(false)
}

/// Log a message to the console (works in both browser and Node.js)
pub fn console_log(level: &str, message: &str) {
    let _ = Reflect::get(&js_sys::global(), &"console".into())
        .and_then(|console| {
            let log_fn = Reflect::get(&console, &level.into())?;
            if let Ok(func) = log_fn.dyn_into::<js_sys::Function>() {
                let _ = func.call1(&JsValue::NULL, &JsValue::from_str(message));
            }
            Ok(JsValue::UNDEFINED)
        });
}

/// Get current timestamp in milliseconds (works in both browser and Node.js)
pub fn get_timestamp_ms() -> u64 {
    js_sys::Date::now() as u64
}

/// Get current timestamp in seconds (works in both browser and Node.js)
pub fn get_timestamp_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

/// Sleep for a specified number of milliseconds (works in both browser and Node.js)
pub async fn sleep_ms(ms: u64) -> Result<()> {
    let global = js_sys::global();

    // Create a promise that resolves after the timeout
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        // Try to use setTimeout from globalThis (works in both browser and Node.js)
        let set_timeout = Reflect::get(&global, &"setTimeout".into())
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok());

        if let Some(set_timeout_fn) = set_timeout {
            let _ = set_timeout_fn.call2(
                &JsValue::NULL,
                &resolve,
                &JsValue::from_f64(ms as f64)
            );
        } else {
            // If setTimeout not available, resolve immediately
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }
    });

    JsFuture::from(promise)
        .await
        .map_err(|e| AlkanesError::Io(format!("Sleep failed: {:?}", e)))?;

    Ok(())
}

/// Platform-agnostic storage interface
///
/// In browser: uses localStorage
/// In Node.js: uses in-memory storage (for tests)
///
/// Uses RefCell with try_borrow for WASM-compatible concurrent access.
/// Operations clone data immediately to avoid holding borrows across async points.
pub struct PlatformStorage {
    // In-memory fallback for Node.js - uses RefCell for single-threaded WASM
    memory_storage: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, String>>>,
    is_browser: bool,
}

impl PlatformStorage {
    pub fn new() -> Self {
        Self {
            memory_storage: std::rc::Rc::new(std::cell::RefCell::new(std::collections::HashMap::new())),
            is_browser: is_browser(),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let prefixed_key = format!("alkanes:{}", key);

        if self.is_browser {
            // Browser: use localStorage via web_sys
            web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item(&prefixed_key).ok())
                .flatten()
        } else {
            // Node.js: use in-memory storage
            // Use try_borrow to avoid panicking on concurrent access
            match self.memory_storage.try_borrow() {
                Ok(guard) => guard.get(&prefixed_key).cloned(),
                Err(_) => {
                    // If we can't borrow, the storage is being modified - return None
                    // This is safe because we're in a single-threaded environment
                    None
                }
            }
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let prefixed_key = format!("alkanes:{}", key);

        if self.is_browser {
            // Browser: use localStorage via web_sys
            let storage = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .ok_or_else(|| AlkanesError::Storage("localStorage not available".to_string()))?;

            storage.set_item(&prefixed_key, value)
                .map_err(|e| AlkanesError::Storage(format!("Failed to set item: {:?}", e)))
        } else {
            // Node.js: use in-memory storage
            // Use try_borrow_mut to avoid panicking on concurrent access
            match self.memory_storage.try_borrow_mut() {
                Ok(mut guard) => {
                    guard.insert(prefixed_key, value.to_string());
                    Ok(())
                }
                Err(_) => {
                    Err(AlkanesError::Storage("Storage is currently in use".to_string()))
                }
            }
        }
    }

    pub fn remove(&self, key: &str) -> Result<()> {
        let prefixed_key = format!("alkanes:{}", key);

        if self.is_browser {
            // Browser: use localStorage via web_sys
            let storage = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .ok_or_else(|| AlkanesError::Storage("localStorage not available".to_string()))?;

            storage.remove_item(&prefixed_key)
                .map_err(|e| AlkanesError::Storage(format!("Failed to remove item: {:?}", e)))
        } else {
            // Node.js: use in-memory storage
            match self.memory_storage.try_borrow_mut() {
                Ok(mut guard) => {
                    guard.remove(&prefixed_key);
                    Ok(())
                }
                Err(_) => {
                    Err(AlkanesError::Storage("Storage is currently in use".to_string()))
                }
            }
        }
    }

    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn list_keys(&self, prefix: &str) -> Vec<String> {
        let full_prefix = format!("alkanes:{}", prefix);

        if self.is_browser {
            // Browser: iterate localStorage
            let mut keys = Vec::new();
            if let Some(storage) = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
            {
                if let Ok(length) = storage.length() {
                    for i in 0..length {
                        if let Ok(Some(key)) = storage.key(i) {
                            if key.starts_with(&full_prefix) {
                                if let Some(stripped) = key.strip_prefix("alkanes:") {
                                    keys.push(stripped.to_string());
                                }
                            }
                        }
                    }
                }
            }
            keys
        } else {
            // Node.js: iterate in-memory storage
            match self.memory_storage.try_borrow() {
                Ok(guard) => {
                    guard.keys()
                        .filter(|k| k.starts_with(&full_prefix))
                        .filter_map(|k| k.strip_prefix("alkanes:").map(|s| s.to_string()))
                        .collect()
                }
                Err(_) => Vec::new()
            }
        }
    }
}

impl Default for PlatformStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PlatformStorage {
    fn clone(&self) -> Self {
        Self {
            memory_storage: std::rc::Rc::clone(&self.memory_storage),
            is_browser: self.is_browser,
        }
    }
}

/// Perform a fetch request (works in both browser and Node.js)
///
/// This function uses the global fetch API which is available in:
/// - Modern browsers (globalThis.fetch)
/// - Node.js 18+ (globalThis.fetch)
/// - Test environments with fetch polyfills
pub async fn fetch(url: &str, method: &str, body: Option<&str>, headers: Vec<(&str, &str)>) -> Result<String> {
    use js_sys::{Object, Reflect};

    let global = js_sys::global();

    // Always use global fetch - it works in both browsers and Node.js 18+
    {
        let opts = Object::new();
        Reflect::set(&opts, &"method".into(), &JsValue::from_str(method))
            .map_err(|_| AlkanesError::Network("Failed to set method".to_string()))?;
        
        if let Some(body_str) = body {
            Reflect::set(&opts, &"body".into(), &JsValue::from_str(body_str))
                .map_err(|_| AlkanesError::Network("Failed to set body".to_string()))?;
        }
        
        // Set headers
        let headers_obj = Object::new();
        for (key, value) in headers {
            Reflect::set(&headers_obj, &JsValue::from_str(key), &JsValue::from_str(value))
                .map_err(|_| AlkanesError::Network("Failed to set header".to_string()))?;
        }
        Reflect::set(&opts, &"headers".into(), &headers_obj)
            .map_err(|_| AlkanesError::Network("Failed to set headers object".to_string()))?;
        
        // Call global fetch (available in Node.js)
        let fetch_fn = Reflect::get(&global, &"fetch".into())
            .map_err(|_| AlkanesError::Network("fetch not available in global scope".to_string()))?;
        
        let fetch_fn = fetch_fn.dyn_into::<js_sys::Function>()
            .map_err(|_| AlkanesError::Network("fetch is not a function".to_string()))?;
        
        let promise = fetch_fn.call2(&JsValue::NULL, &JsValue::from_str(url), &opts)
            .map_err(|e| AlkanesError::Network(format!("fetch call failed: {:?}", e)))?;
        
        let resp_value = JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|e| AlkanesError::Network(format!("fetch promise failed: {:?}", e)))?;
        
        // Check status
        let ok = Reflect::get(&resp_value, &"ok".into())
            .map_err(|_| AlkanesError::Network("No ok property".to_string()))?;
        
        if !ok.is_truthy() {
            let status = Reflect::get(&resp_value, &"status".into())
                .and_then(|s| Ok(s.as_f64().unwrap_or(0.0) as u16))
                .unwrap_or(0);
            return Err(AlkanesError::Network(format!("HTTP error: {}", status)));
        }
        
        // Get text from response
        let text_method = Reflect::get(&resp_value, &"text".into())
            .map_err(|_| AlkanesError::Network("No text method".to_string()))?;
        
        let text_fn = text_method.dyn_into::<js_sys::Function>()
            .map_err(|_| AlkanesError::Network("text is not a function".to_string()))?;
        
        let text_promise = text_fn.call0(&resp_value)
            .map_err(|e| AlkanesError::Network(format!("text call failed: {:?}", e)))?;
        
        let text_value = JsFuture::from(js_sys::Promise::from(text_promise))
            .await
            .map_err(|e| AlkanesError::Network(format!("text promise failed: {:?}", e)))?;
        
        text_value.as_string()
            .ok_or_else(|| AlkanesError::Network("Response is not a string".to_string()))
    }
}
