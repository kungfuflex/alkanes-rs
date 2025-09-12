#![cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "test-utils")]
use once_cell::sync::Lazy;
#[cfg(feature = "test-utils")]
use std::sync::Mutex;

#[cfg(feature = "test-utils")]
pub static _INPUT: Lazy<Mutex<Option<Vec<u8>>>> = Lazy::new(|| Mutex::new(None));

#[cfg(feature = "test-utils")]
pub fn set_input(input: Vec<u8>) {
    *_INPUT.lock().unwrap() = Some(input);
}

// These are the functions that are imported from the host environment
// They are defined in the metashrew-host crate
// The wasm-bindgen attribute is used to generate the JavaScript glue code
// that allows the WebAssembly module to call these functions
// The js_namespace attribute is used to specify the JavaScript namespace
// where these functions are located
// The js_name attribute is used to specify the JavaScript name of the function
// The catch attribute is used to catch JavaScript exceptions and convert them
// into a Result
// The final attribute is used to specify that the function is final and cannot
// be overridden
// The module attribute is used to specify the JavaScript module where the
// function is located
// The import_name attribute is used to specify the name of the function in the
// JavaScript module
// The link_name attribute is used to specify the name of the function in the
d // WebAssembly module
// The wasm_import_module attribute is used to specify the name of the
// WebAssembly module
// The wasm_import_name attribute is used to specify the name of the function
// in the WebAssembly module

#[wasm_bindgen]
extern "C" {
    pub fn __get(key_ptr: i32, value_ptr: i32);
    pub fn __get_len(key_ptr: i32) -> i32;
    pub fn __set(key_ptr: i32, value_ptr: i32);
    pub fn __flush(ptr: i32);
    pub fn __host_len() -> i32;
    pub fn __load_input(ptr: i32);
}

#[wasm_bindgen(js_namespace = Date)]
extern "C" {
    pub fn now() -> f64;
}

#[wasm_bindgen(js_namespace = console)]
extern "C" {
    pub fn log(s: &str);
}

#[wasm_bindgen(js_namespace = ["process", "stdout"])]
extern "C" {
    pub fn write(s: &str);
}
