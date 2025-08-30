use std::sync::Arc;
use std::cell::RefCell;

thread_local! {
    static NATIVE_PTR_CHANNEL: RefCell<Option<Vec<u8>>> = RefCell::new(None);
}

#[cfg(target_arch = "wasm32")]
pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    return v.as_mut_ptr() as usize as i32;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    NATIVE_PTR_CHANNEL.with(|channel| {
        *channel.borrow_mut() = Some(v.clone());
    });
    0 // Return a dummy pointer
}


#[cfg(target_arch = "wasm32")]
pub fn to_passback_ptr(v: &mut Vec<u8>) -> i32 {
    to_ptr(v) + 4
}

#[cfg(not(target_arch = "wasm32"))]
pub fn to_passback_ptr(v: &mut Vec<u8>) -> i32 {
    NATIVE_PTR_CHANNEL.with(|channel| {
        *channel.borrow_mut() = Some(v.clone());
    });
    0 // Return a dummy pointer
}


pub fn take_native_ptr() -> Option<Vec<u8>> {
    NATIVE_PTR_CHANNEL.with(|channel| channel.borrow_mut().take())
}


pub fn to_arraybuffer_layout<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    buffer.extend_from_slice(&(v.as_ref().len() as u32).to_le_bytes());
    buffer.extend_from_slice(v.as_ref());
    return buffer;
}

#[allow(unused_unsafe)]
#[cfg(all(not(feature = "test-utils"), target_arch = "wasm32"))]
pub fn log(v: Arc<Vec<u8>>) -> () {
    #[link(wasm_import_module = "env")]
    extern "C" {
        fn __log(ptr: i32);
    }
    unsafe {
        let mut buffer = to_arraybuffer_layout(v.as_ref());
        __log(to_passback_ptr(&mut buffer));
    }
}
#[allow(unused_unsafe)]
#[cfg(all(feature = "test-utils", target_arch = "wasm32"))]
pub fn log(v: Arc<Vec<u8>>) -> () {
  use wasm_bindgen::prelude::*;
  #[wasm_bindgen(js_namespace = ["process", "stdout"])]
  extern "C" {
    fn write(s: &str);
  }
  unsafe {
    write(format!("{}", String::from_utf8(v.as_ref().to_vec()).unwrap()).as_str());
  }
}

#[allow(unused_unsafe)]
#[cfg(not(target_arch = "wasm32"))]
pub fn log(_v: Arc<Vec<u8>>) -> () {
}