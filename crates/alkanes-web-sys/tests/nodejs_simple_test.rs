//! Simple Node.js Test
//!
//! This test verifies basic functionality works in Node.js environment

use alkanes_web_sys::WebProvider;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use wasm_bindgen_futures::JsFuture;

// Helper macro for logging that works in Node.js
macro_rules! log {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        let _ = js_sys::Reflect::get(&js_sys::global(), &"console".into())
            .and_then(|console| {
                let log_fn = js_sys::Reflect::get(&console, &"log".into())?;
                let log_fn = log_fn.dyn_into::<js_sys::Function>()?;
                log_fn.call1(&JsValue::NULL, &JsValue::from_str(&msg))
            });
    };
}

#[wasm_bindgen_test]
fn test_platform_detection() {
    log!("=== Test: Platform Detection ===");

    let global = js_sys::global();
    let has_window = js_sys::Reflect::has(&global, &"window".into()).unwrap_or(false);
    let has_fetch = js_sys::Reflect::has(&global, &"fetch".into()).unwrap_or(false);
    let has_console = js_sys::Reflect::has(&global, &"console".into()).unwrap_or(false);
    let has_set_timeout = js_sys::Reflect::has(&global, &"setTimeout".into()).unwrap_or(false);

    log!("   has_window: {}", has_window);
    log!("   has_fetch: {}", has_fetch);
    log!("   has_console: {}", has_console);
    log!("   has_setTimeout: {}", has_set_timeout);

    // In Node.js, we should NOT have window but SHOULD have fetch, console, setTimeout
    assert!(!has_window, "Should not have window in Node.js");
    assert!(has_fetch, "Should have fetch in Node.js");
    assert!(has_console, "Should have console in Node.js");

    log!("✅ Platform detection passed!");
}

#[wasm_bindgen_test]
fn test_provider_creation() {
    log!("=== Test: Provider Creation ===");

    let result = WebProvider::new_js("subfrost-regtest".to_string(), None);

    match result {
        Ok(provider) => {
            log!("✅ Provider created successfully");
            // Just check that we can get some basic info
            log!("   Provider network: subfrost-regtest");
        }
        Err(e) => {
            log!("❌ Provider creation failed: {:?}", e);
            panic!("Provider creation failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_simple_fetch() {
    log!("=== Test: Simple Fetch ===");

    // Test that we can make a basic fetch request using the global fetch
    let global = js_sys::global();

    let fetch_fn = js_sys::Reflect::get(&global, &"fetch".into())
        .expect("fetch should exist");

    let fetch_fn = fetch_fn.dyn_into::<js_sys::Function>()
        .expect("fetch should be a function");

    log!("   Attempting to fetch from httpbin...");

    // Make a simple GET request
    let promise = fetch_fn.call1(&JsValue::NULL, &JsValue::from_str("https://httpbin.org/get"))
        .expect("fetch call should succeed");

    let result = JsFuture::from(js_sys::Promise::from(promise)).await;

    match result {
        Ok(resp) => {
            let ok = js_sys::Reflect::get(&resp, &"ok".into())
                .map(|v| v.is_truthy())
                .unwrap_or(false);

            let status = js_sys::Reflect::get(&resp, &"status".into())
                .and_then(|s| Ok(s.as_f64().unwrap_or(0.0) as u16))
                .unwrap_or(0);

            log!("   Response ok: {}, status: {}", ok, status);

            if ok {
                log!("✅ Simple fetch succeeded!");
            } else {
                log!("⚠️ Fetch returned non-ok status");
            }
        }
        Err(e) => {
            log!("❌ Fetch failed: {:?}", e);
            // Don't panic - network may not be available in test environment
        }
    }
}

#[wasm_bindgen_test]
async fn test_regtest_fetch() {
    log!("=== Test: Regtest RPC Fetch ===");

    let global = js_sys::global();

    let fetch_fn = js_sys::Reflect::get(&global, &"fetch".into())
        .expect("fetch should exist")
        .dyn_into::<js_sys::Function>()
        .expect("fetch should be a function");

    // Create request options
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &"method".into(), &JsValue::from_str("POST")).unwrap();

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "metashrew_height",
        "params": [],
        "id": 1
    });
    js_sys::Reflect::set(&opts, &"body".into(), &JsValue::from_str(&body.to_string())).unwrap();

    let headers = js_sys::Object::new();
    js_sys::Reflect::set(&headers, &"Content-Type".into(), &JsValue::from_str("application/json")).unwrap();
    js_sys::Reflect::set(&opts, &"headers".into(), &headers).unwrap();

    let url = "https://regtest.subfrost.io/v4/subfrost";
    log!("   Fetching from: {}", url);

    let promise = fetch_fn.call2(&JsValue::NULL, &JsValue::from_str(url), &opts)
        .expect("fetch call should succeed");

    let result = JsFuture::from(js_sys::Promise::from(promise)).await;

    match result {
        Ok(resp) => {
            let status = js_sys::Reflect::get(&resp, &"status".into())
                .and_then(|s| Ok(s.as_f64().unwrap_or(0.0) as u16))
                .unwrap_or(0);

            log!("   Response status: {}", status);

            // Try to get the response body
            let text_fn = js_sys::Reflect::get(&resp, &"text".into())
                .and_then(|t| t.dyn_into::<js_sys::Function>())
                .ok();

            if let Some(text_fn) = text_fn {
                let text_promise = text_fn.call0(&resp).ok();
                if let Some(text_promise) = text_promise {
                    if let Ok(text_val) = JsFuture::from(js_sys::Promise::from(text_promise)).await {
                        if let Some(text) = text_val.as_string() {
                            log!("   Response body (first 200 chars): {}", &text[..text.len().min(200)]);
                        }
                    }
                }
            }

            log!("✅ Regtest fetch completed!");
        }
        Err(e) => {
            log!("❌ Regtest fetch failed: {:?}", e);
            panic!("Regtest fetch failed: {:?}", e);
        }
    }
}

#[wasm_bindgen_test]
async fn test_provider_metashrew_height() {
    log!("=== Test: Provider metashrew_height ===");

    let provider = WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Provider should be created");

    log!("   Calling metashrew_height_js...");

    let result = JsFuture::from(provider.metashrew_height_js()).await;

    match result {
        Ok(height) => {
            log!("✅ metashrew_height succeeded: {:?}", height);
        }
        Err(e) => {
            log!("❌ metashrew_height failed: {:?}", e);
            // Don't panic yet - let's see what the error is
        }
    }
}

#[wasm_bindgen_test]
async fn test_provider_data_api_pools() {
    log!("=== Test: Provider data_api_get_pools ===");

    let provider = WebProvider::new_js("subfrost-regtest".to_string(), None)
        .expect("Provider should be created");

    log!("   Calling data_api_get_pools_js...");

    let result = JsFuture::from(provider.data_api_get_pools_js("4:65522".to_string())).await;

    match result {
        Ok(pools) => {
            log!("✅ data_api_get_pools succeeded: {:?}", pools);
        }
        Err(e) => {
            log!("❌ data_api_get_pools failed: {:?}", e);
        }
    }
}
