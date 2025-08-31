use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

pub trait StorageAdapter: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, anyhow::Error>;
}

thread_local! {
    static LAST_GET_VALUE: RefCell<Option<Vec<u8>>> = RefCell::new(None);
    static INPUT: RefCell<Option<Vec<u8>>> = RefCell::new(None);
}

pub static STORAGE_ADAPTER: Lazy<Mutex<Option<Arc<dyn StorageAdapter>>>> = Lazy::new(|| Mutex::new(None));

pub fn set_storage_adapter(adapter: Arc<dyn StorageAdapter>) {
    *STORAGE_ADAPTER.lock().unwrap() = Some(adapter);
}

pub fn set_input(input: Vec<u8>) {
    INPUT.with(|v| *v.borrow_mut() = Some(input));
}

pub fn host_len() -> i32 {
    INPUT.with(|v| {
        if let Some(input) = v.borrow().as_ref() {
            input.len() as i32
        } else {
            0
        }
    })
}

pub fn flush(_ptr: i32) {
    // In the native implementation, flushing is handled by the sync engine.
}

pub fn get(_key_ptr: i32, _value_ptr: i32) {
    let mut dest = crate::wasm::take_native_ptr().expect("Destination buffer missing in __get");
    let value = LAST_GET_VALUE.with(|v| v.borrow_mut().take()).expect("Value missing in __get");
    
    let value_len = value.len();
    dest[4..4+value_len].copy_from_slice(&value);

    crate::wasm::to_passback_ptr(&mut dest);
}

pub fn get_len(key_ptr: i32) -> i32 {
    let key = crate::utils::ptr_to_vec(key_ptr);
    if let Some(adapter) = STORAGE_ADAPTER.lock().unwrap().as_ref() {
        if let Ok(Some(value)) = adapter.get(&key) {
            let len = value.len() as i32;
            LAST_GET_VALUE.with(|v| *v.borrow_mut() = Some(value));
            return len;
        }
    }
    LAST_GET_VALUE.with(|v| *v.borrow_mut() = Some(vec![]));
    0
}

pub fn load_input(ptr: i32) {
    let mut dest = crate::wasm::take_native_ptr().expect("Destination buffer missing in __load_input");
    let input = INPUT.with(|v| v.borrow().clone()).expect("Input missing in __load_input");
    dest.copy_from_slice(&input);
    crate::wasm::to_passback_ptr(&mut dest);
}

pub fn log(ptr: i32) {
    let msg = crate::utils::ptr_to_vec(ptr);
    println!("[WASM LOG]: {}", String::from_utf8_lossy(&msg));
}