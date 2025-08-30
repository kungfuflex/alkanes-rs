#[cfg(feature = "test-utils")]
use wasm_bindgen::prelude::*;

#[allow(unused_imports)]
use crate::utils::ptr_to_vec;
pub static mut _INPUT: Option<Vec<u8>> = None;

#[allow(static_mut_refs)]
#[cfg(feature = "test-utils")]
pub fn __set_input(v: Vec<u8>) {
    unsafe {
        _INPUT = Some(v);
    }
}

#[cfg(not(feature = "test-utils"))]
#[link(wasm_import_module = "env")]
extern "C" {
    pub fn __host_len() -> i32;
    pub fn __flush(ptr: i32);
    pub fn __get(ptr: i32, v: i32);
    pub fn __get_len(ptr: i32) -> i32;
    pub fn __load_input(ptr: i32);
    pub fn __log(ptr: i32);
}

#[allow(static_mut_refs)]
#[cfg(feature = "test-utils")]
pub fn __host_len() -> i32 {
    unsafe {
        match _INPUT.as_ref() {
            Some(v) => v.len() as i32,
            None => 0,
        }
    }
}

#[allow(static_mut_refs)]
#[cfg(feature = "test-utils")]
pub fn __load_input(_ptr: i32) -> () {
    // In test mode, we don't actually write to memory via raw pointers
    // The input() function will use __host_len() to get the length
    // and this function is just a no-op for safety
}

#[cfg(feature = "test-utils")]
pub fn __get_len(_ptr: i32) -> i32 {
    // Return 0 to indicate no data found in "database"
    // This simulates a cache miss at the host level
    0
}

#[cfg(feature = "test-utils")]
pub fn __flush(_ptr: i32) -> () {
    // No-op for tests - just simulate successful flush
}

#[cfg(feature = "test-utils")]
pub fn __get(_ptr: i32, _result: i32) -> () {
    // No-op for tests - since __get_len returns 0, this shouldn't be called
    // But if it is called, we don't write anything to the result buffer
}

#[cfg(feature = "test-utils")]
#[wasm_bindgen(js_namespace = Date)]
extern "C" {
    fn now() -> f64;
}

#[cfg(feature = "test-utils")]
pub fn __now() -> u64 {
    now() as u64
}

#[cfg(feature = "test-utils")]
#[wasm_bindgen(js_namespace = ["process", "stdout"])]
extern "C" {
    fn write(s: &str);
}

#[cfg(feature = "test-utils")]
pub fn __log(ptr: i32) -> () {
    write(format!("{}", String::from_utf8(ptr_to_vec(ptr)).unwrap()).as_str());
}
