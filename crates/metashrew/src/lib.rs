extern crate alloc;
use protobuf::Message;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::fmt::Write;
#[cfg(feature = "panic-hook")]
use std::panic;
use std::sync::Arc;

// Re-export metashrew-lib
pub use metashrew_lib as lib;

#[cfg(feature = "panic-hook")]
pub mod compat;
pub mod imports;
pub mod index_pointer;
pub mod proto;
pub mod stdio;
#[cfg(test)]
pub mod tests;

#[cfg(feature = "panic-hook")]
use crate::compat::panic_hook;
use crate::imports::{__flush, __get, __get_len, __host_len, __load_input};
use crate::proto::metashrew::KeyValueFlush;
pub use crate::stdio::stdout;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr};

// For backward compatibility, we'll keep the existing API but implement it using metashrew-lib
static mut CACHE: Option<HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>> = None;
static mut TO_FLUSH: Option<Vec<Arc<Vec<u8>>>> = None;

#[allow(static_mut_refs)]
pub fn get_cache() -> &'static HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
    unsafe { CACHE.as_ref().unwrap() }
}

#[allow(static_mut_refs)]
pub fn get(v: Arc<Vec<u8>>) -> Arc<Vec<u8>> {
    unsafe {
        initialize();
        if CACHE.as_ref().unwrap().contains_key(&v.clone()) {
            return CACHE.as_ref().unwrap().get(&v.clone()).unwrap().clone();
        }
        
        // Use metashrew-lib's host functions
        let result = metashrew_lib::host::get(v.as_ref()).unwrap_or_default();
        let value = Arc::new(result);
        CACHE.as_mut().unwrap().insert(v.clone(), value.clone());
        value
    }
}

#[allow(static_mut_refs)]
pub fn set(k: Arc<Vec<u8>>, v: Arc<Vec<u8>>) {
    unsafe {
        initialize();
        CACHE.as_mut().unwrap().insert(k.clone(), v.clone());
        TO_FLUSH.as_mut().unwrap().push(k.clone());
    }
}

#[allow(static_mut_refs)]
pub fn flush() {
    unsafe {
        initialize();
        let mut pairs = Vec::new();
        for item in TO_FLUSH.as_ref().unwrap() {
            pairs.push((
                (*item.clone()).clone(),
                (*(CACHE.as_ref().unwrap().get(item).unwrap().clone())).clone(),
            ));
        }
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());
        
        // Use metashrew-lib's host functions
        let _ = metashrew_lib::host::flush(&pairs);
    }
}

#[allow(unused_unsafe)]
pub fn input() -> Vec<u8> {
    initialize();
    
    // Use metashrew-lib's host functions
    let (height, block) = metashrew_lib::host::load_input().unwrap_or_default();
    
    // Format the result the same way as the original function
    let mut result = Vec::new();
    result.extend_from_slice(&height.to_le_bytes());
    result.extend_from_slice(&block);
    result[4..].to_vec()
}

#[allow(static_mut_refs)]
pub fn initialize() -> () {
    unsafe {
        if CACHE.is_none() {
            reset();
            CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
            #[cfg(feature = "panic-hook")]
            panic::set_hook(Box::new(panic_hook));
        }
    }
}

pub fn reset() -> () {
    unsafe {
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());
    }
}

pub fn clear() -> () {
    unsafe {
        reset();
        CACHE = Some(HashMap::<Arc<Vec<u8>>, Arc<Vec<u8>>>::new());
    }
}

// Add a println macro that uses metashrew-lib's log function
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use std::fmt::Write;
        let mut s = String::new();
        write!(&mut s, $($arg)*).unwrap();
        metashrew_lib::host::log(&s);
    }};
}
