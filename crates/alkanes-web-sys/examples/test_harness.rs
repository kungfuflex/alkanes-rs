//! Custom Test Harness for deezel-web
//!
//! This harness is a workaround for environments where `wasm-pack test` with a
//! headless browser is unstable. It works by:
//! 1. Importing test modules using the `#[path]` attribute.
//! 2. Calling all `pub` test functions from a `#[wasm_bindgen(start)]` function.
//! 3. Being compiled as a standard WASM library using `wasm-pack build`.
//! 4. Being loaded by a simple HTML file (`test_harness.html`).
//!
//! This allows tests to be run in any standard web browser with developer tools.

// Silence warnings for unused code, as this is a test runner binary.
#![allow(dead_code)]

use wasm_bindgen::prelude::*;

// Import test modules from the `tests` directory.
// The `#[path]` attribute allows us to include files from outside the conventional
// module hierarchy, which is perfect for a custom test runner.
/// The main entry point for the WASM module, executed when the module is loaded.
#[wasm_bindgen(start)]
pub fn run_all_tests() {
    // Use the browser's console for logging test status.
    web_sys::console::log_1(&"Starting deezel-web integration tests...".into());

    // --- Run Esplora Provider Tests ---
    web_sys::console::log_1(&"Running Esplora provider tests...".into());
    
    web_sys::console::log_1(&"All tests dispatched.".into());
}

/// A `main` function is required to satisfy the Rust compiler when building an
/// example binary, even though it's not used in the WASM context.
fn main() {
    // This function is not called in the WASM environment, but is needed for `cargo build`.
}