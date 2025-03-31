extern crate alloc;
use protobuf::Message;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::fmt::Write;
#[cfg(feature = "panic-hook")]
use std::panic;
use std::sync::Arc;

#[cfg(feature = "panic-hook")]
use metashrew_support::compat::panic_hook;

use crate::host::{__flush, __get, __get_len, __host_len, __load_input};
use metashrew_support::proto::metashrew::KeyValueFlush;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr};

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
        let length: i32 = __get_len(to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())));
        let mut buffer = Vec::<u8>::new();
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.resize((length as usize) + 4, 0);
        __get(
            to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())),
            to_passback_ptr(&mut buffer),
        );
        let value = Arc::new(buffer[4..].to_vec());
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
        let mut to_encode: Vec<Vec<u8>> = Vec::<Vec<u8>>::new();
        for item in TO_FLUSH.as_ref().unwrap() {
            to_encode.push((*item.clone()).clone());
            to_encode.push((*(CACHE.as_ref().unwrap().get(item).unwrap().clone())).clone());
        }
        TO_FLUSH = Some(Vec::<Arc<Vec<u8>>>::new());
        let mut buffer = KeyValueFlush::new();
        buffer.list = to_encode;
        let serialized = buffer.write_to_bytes().unwrap();
        __flush(to_ptr(&mut to_arraybuffer_layout(&serialized.to_vec())) + 4);
    }
}

#[allow(unused_unsafe)]
pub fn input() -> Vec<u8> {
    initialize();
    unsafe {
        let length: i32 = __host_len().into();
        let mut buffer = Vec::<u8>::new();
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.resize((length as usize) + 4, 0);
        __load_input(to_ptr(&mut buffer) + 4);
        buffer[4..].to_vec()
    }
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
