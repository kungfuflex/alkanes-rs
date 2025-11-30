//! Platform abstraction layer for WASM bindings
//!
//! This module provides functions that abstract platform-specific functionality,
//! allowing tests to run in different environments (browser, Node.js, etc.)

use alkanes_cli_common::{AlkanesError, Result};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Perform a fetch request (works in both browser and Node.js)
pub async fn fetch(url: &str, method: &str, body: Option<&str>, headers: Vec<(&str, &str)>) -> Result<String> {
    // Try Node.js/global fetch first (for tests), fall back to web_sys (for browser)
    use js_sys::{Object, Reflect};
    
    // Check if we're in a browser environment by looking for window
    let global = js_sys::global();
    let has_window = Reflect::has(&global, &"window".into()).unwrap_or(false);
    
    if !has_window {
        // Node.js mode: use global fetch
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
    } else {
        // Browser mode: use web_sys
        use web_sys::{window, Request, RequestInit, RequestMode, Response};
        
        let window = window().ok_or_else(|| AlkanesError::Network("No window object available".to_string()))?;
        
        let mut opts = RequestInit::new();
        opts.method(method);
        opts.mode(RequestMode::Cors);
        
        if let Some(body_str) = body {
            opts.body(Some(&JsValue::from_str(body_str)));
        }
        
        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| AlkanesError::Network(format!("Failed to create request: {:?}", e)))?;
        
        // Set headers
        let req_headers = request.headers();
        for (key, value) in headers {
            req_headers.set(key, value)
                .map_err(|e| AlkanesError::Network(format!("Failed to set header: {:?}", e)))?;
        }
        
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| AlkanesError::Network(format!("Fetch failed: {:?}", e)))?;
        
        let resp: Response = resp_value.dyn_into()
            .map_err(|e| AlkanesError::Network(format!("Response conversion failed: {:?}", e)))?;
        
        if !resp.ok() {
            return Err(AlkanesError::Network(format!("HTTP error: {}", resp.status())));
        }
        
        let text = JsFuture::from(resp.text()
            .map_err(|e| AlkanesError::Network(format!("Failed to get text: {:?}", e)))?)
            .await
            .map_err(|e| AlkanesError::Network(format!("Text conversion failed: {:?}", e)))?;
        
        text.as_string()
            .ok_or_else(|| AlkanesError::Network("Response text is not a string".to_string()))
    }
}
