#[cfg(feature = "std")]
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use crate::environment::EnvironmentInput;
use std::vec;
use std::vec::Vec;
use std::string::String;

static MOCK_STORAGE: Lazy<Mutex<HashMap<Vec<u8>, Vec<u8>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

static MOCK_INPUT: Lazy<Mutex<Vec<u8>>> = Lazy::new(|| {
    let input = EnvironmentInput {
        height: 0,
        data: vec![],
    };
    Mutex::new(input.to_bytes())
});

#[no_mangle]
pub unsafe extern "C" fn __get_len(key_ptr: *const u8, key_len: usize) -> i32 {
    let key = core::slice::from_raw_parts(key_ptr, key_len);
    let storage = MOCK_STORAGE.lock().unwrap();
    if let Some(value) = storage.get(key) {
        value.len() as i32
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn __get(key_ptr: *const u8, key_len: usize, value_ptr: *mut u8) {
    let key = core::slice::from_raw_parts(key_ptr, key_len);
    let storage = MOCK_STORAGE.lock().unwrap();
    if let Some(value) = storage.get(key) {
        core::ptr::copy_nonoverlapping(value.as_ptr(), value_ptr, value.len());
    }
}

#[no_mangle]
pub unsafe extern "C" fn __flush(ptr: *const u8, len: usize) {
    let data = core::slice::from_raw_parts(ptr, len);
    let mut storage = MOCK_STORAGE.lock().unwrap();
    let mut i = 0;
    while i < data.len() {
        let key_len = u32::from_le_bytes(data[i..i+4].try_into().unwrap()) as usize;
        i += 4;
        let key = &data[i..i+key_len];
        i += key_len;
        let value_len = u32::from_le_bytes(data[i..i+4].try_into().unwrap()) as usize;
        i += 4;
        let value = &data[i..i+value_len];
        i += value_len;
        storage.insert(key.to_vec(), value.to_vec());
    }
}

#[no_mangle]
pub unsafe extern "C" fn __host_len() -> usize {
    MOCK_INPUT.lock().unwrap().len()
}

#[no_mangle]
pub unsafe extern "C" fn __load_input(ptr: *mut u8) {
    let input = MOCK_INPUT.lock().unwrap();
    core::ptr::copy_nonoverlapping(input.as_ptr(), ptr, input.len());
}

#[no_mangle]
pub unsafe extern "C" fn __log(ptr: *const u8, len: usize) {
    let message = core::slice::from_raw_parts(ptr, len);
    println!("{}", String::from_utf8_lossy(message));
}

#[no_mangle]
pub unsafe extern "C" fn __clear() {
    MOCK_STORAGE.lock().unwrap().clear();
}