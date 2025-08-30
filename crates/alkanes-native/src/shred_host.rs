use once_cell::sync::Lazy;
use std::sync::Mutex;
use crate::adapters::RocksDBAdapter;
use std::cell::RefCell;

thread_local! {
    static LAST_GET_VALUE: RefCell<Option<Vec<u8>>> = RefCell::new(None);
}

pub static STORAGE_ADAPTER: Lazy<Mutex<Option<RocksDBAdapter>>> = Lazy::new(|| Mutex::new(None));

pub fn set_storage_adapter(adapter: RocksDBAdapter) {
    *STORAGE_ADAPTER.lock().unwrap() = Some(adapter);
}

#[no_mangle]
pub extern "C" fn __host_len() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __flush(_ptr: i32) {
    // In the native implementation, flushing is handled by the sync engine.
}

#[no_mangle]
pub extern "C" fn __get(_key_ptr: i32, _value_ptr: i32) {
    let mut dest = metashrew_core::wasm::take_native_ptr().expect("Destination buffer missing in __get");
    let value = LAST_GET_VALUE.with(|v| v.borrow_mut().take()).expect("Value missing in __get");
    
    // The `get` function in metashrew-core expects the buffer to be filled.
    // We need to copy the value into the destination buffer.
    // The destination buffer from the channel already has the length prefix.
    let value_len = value.len();
    dest[4..4+value_len].copy_from_slice(&value);

    // We need to put the modified buffer back for the caller to find.
    // This is a bit of a hack, but it mirrors how the WASM memory would be modified in place.
    metashrew_core::wasm::to_passback_ptr(&mut dest);
}

#[no_mangle]
pub extern "C" fn __get_len(key_ptr: i32) -> i32 {
    let key = metashrew_core::utils::ptr_to_vec(key_ptr);
    if let Some(adapter) = STORAGE_ADAPTER.lock().unwrap().as_ref() {
        if let Ok(Some(value)) = adapter.db.get(&key) {
            let len = value.len() as i32;
            LAST_GET_VALUE.with(|v| *v.borrow_mut() = Some(value));
            return len;
        }
    }
    LAST_GET_VALUE.with(|v| *v.borrow_mut() = Some(vec![]));
    0
}

#[no_mangle]
pub extern "C" fn __load_input(_ptr: i32) {
    // Input is passed directly to process_block_atomic
}

#[no_mangle]
pub extern "C" fn __log(ptr: i32) {
    let msg = metashrew_core::utils::ptr_to_vec(ptr);
    println!("[WASM LOG]: {}", String::from_utf8_lossy(&msg));
}