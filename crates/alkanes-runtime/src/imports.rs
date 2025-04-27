#[cfg(feature = "test-utils")]
use alkanes_support::context::Context;
#[cfg(feature = "test-utils")]
use wasm_bindgen::prelude::*;

#[cfg(not(feature = "test-utils"))]
#[link(wasm_import_module = "env")]
extern "C" {
    pub fn abort(a: i32, b: i32, c: i32, d: i32);
    pub fn __load_storage(k: i32, v: i32) -> i32;
    pub fn __request_storage(k: i32) -> i32;
    pub fn __log(v: i32);
    pub fn __balance(who: i32, what: i32, output: i32);
    pub fn __request_context() -> i32;
    pub fn __load_context(output: i32) -> i32;
    pub fn __sequence(output: i32);
    pub fn __fuel(output: i32);
    pub fn __height(output: i32);
    pub fn __returndatacopy(output: i32);
    pub fn __request_transaction() -> i32;
    pub fn __load_transaction(output: i32);
    pub fn __request_block() -> i32;
    pub fn __load_block(output: i32);
    pub fn __call(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64) -> i32;
    pub fn __staticcall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32;
    pub fn __delegatecall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32;
}

#[cfg(feature = "test-utils")]
pub mod externs {
    pub use wasm_bindgen::prelude::*;
    #[wasm_bindgen(js_namespace = ["process", "stdout"])]
    extern "C" {
        pub fn write(s: &str);
    }
}

#[cfg(feature = "test-utils")]
pub static mut _CONTEXT: Option<Context> = None;

#[cfg(feature = "test-utils")]
mod exports {
    pub use super::externs;
    use super::_CONTEXT;
    use {
        alkanes_support::context::Context,
        metashrew_support::{compat::to_passback_ptr, utils::ptr_to_vec},
    };
    pub fn set_mock_context(context: Context) {
        unsafe {
            _CONTEXT = Some(context);
        }
    }
    pub fn abort(a: i32, b: i32, c: i32, d: i32) -> i32 {
        panic!("abort");
    }
    pub fn __load_storage(k: i32, v: i32) -> i32 {
        v
    }
    pub fn __request_storage(k: i32) -> i32 {
        0
    }
    pub fn __log(ptr: i32) -> () {
        externs::write(format!("{}", String::from_utf8(ptr_to_vec(ptr)).unwrap()).as_str());
    }
    pub fn __balance(who: i32, what: i32, output: i32) -> i32 {
        0
    }
    pub fn __request_context() -> i32 {
        unsafe {
            match _CONTEXT.as_ref() {
                Some(v) => v.serialize().len() as i32,
                None => 0,
            }
        }
    }
    pub fn __load_context(output: i32) {
        unsafe {
            match _CONTEXT.as_ref() {
                Some(v) => {
                    let mut bytes: Vec<u8> = v.serialize();
                    let len = bytes.len();
                    let bytes_ref: &mut [u8] = &mut bytes;
                    (&mut std::slice::from_raw_parts_mut(output as usize as *mut u8, len))
                        .clone_from_slice(&*bytes_ref);
                }
                None => (),
            }
        }
    }
    pub fn __sequence(output: i32) {}
    pub fn __fuel(output: i32) {}
    pub fn __height(output: i32) {}
    pub fn __returndatacopy(output: i32) {}
    pub fn __request_transaction() -> i32 {
        0
    }
    pub fn __load_transaction(output: i32) {}
    pub fn __request_block() -> i32 {
        0
    }
    pub fn __load_block(output: i32) {}
    pub fn __call(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64) -> i32 {
        0
    }
    pub fn __staticcall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32 {
        0
    }
    pub fn __delegatecall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32 {
        0
    }
}

#[cfg(feature = "test-utils")]
pub use exports::*;
