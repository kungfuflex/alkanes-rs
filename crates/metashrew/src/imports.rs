#[cfg(feature = "test-utils")]
use wasm_bindgen::prelude::*;

// Define our own ptr_to_vec function
pub fn ptr_to_vec(ptr: i32) -> Vec<u8> {
    let len_bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, 4) };
    let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
    let data = unsafe { std::slice::from_raw_parts((ptr as usize + 4) as *const u8, len) };
    data.to_vec()
}

static mut _INPUT: Option<Vec<u8>> = None;

#[allow(static_mut_refs)]
#[cfg(feature = "test-utils")]
pub fn __set_input(v: Vec<u8>) {
    unsafe {
        _INPUT = Some(v);
    }
}

// For non-test builds, we'll use the actual host functions
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

// For test builds, we'll provide mock implementations
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
pub fn __load_input(ptr: i32) -> () {
    unsafe {
        match _INPUT.as_ref() {
            Some(v) => (&mut std::slice::from_raw_parts_mut(ptr as usize as *mut u8, v.len()))
                .clone_from_slice(&*v),
            None => (),
        }
    }
}

#[cfg(feature = "test-utils")]
pub fn __get_len(_ptr: i32) -> i32 {
    0
}

#[cfg(feature = "test-utils")]
pub fn __flush(_ptr: i32) -> () {}

#[cfg(feature = "test-utils")]
pub fn __get(_ptr: i32, _result: i32) -> () {}

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
