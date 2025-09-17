pub use metashrew_support::environment::{EnvironmentInput, RuntimeEnvironment};

extern "C" {
    fn __get_len(key_ptr: *const u8, key_len: usize) -> i32;
    fn __get(key_ptr: *const u8, key_len: usize, value_ptr: *mut u8);
    fn __flush(ptr: *const u8, len: usize);
    fn __host_len() -> usize;
    fn __load_input(ptr: *mut u8);
    fn __log(ptr: *const u8, len: usize);
    fn __clear();
}

#[derive(Debug, Clone, Default)]
pub struct MetashrewEnvironment;

impl RuntimeEnvironment for MetashrewEnvironment {
    fn get(key: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let len = __get_len(key.as_ptr(), key.len());
            if len == -1 {
                None
            } else {
                let mut value = vec![0; len as usize];
                __get(key.as_ptr(), key.len(), value.as_mut_ptr());
                Some(value)
            }
        }
    }

    fn flush(data: &[u8]) -> Result<(), ()> {
        unsafe {
            __flush(data.as_ptr(), data.len());
        }
        Ok(())
    }

    fn load_input() -> Result<EnvironmentInput, ()> {
        unsafe {
            let len = __host_len();
            let mut buffer = Vec::with_capacity(len);
            __load_input(buffer.as_mut_ptr());
            buffer.set_len(len);
            Ok(EnvironmentInput::from_bytes(buffer))
        }
    }

    fn log(message: &str) {
        unsafe {
            __log(message.as_ptr(), message.len());
        }
    }

    fn clear() {
        unsafe {
            __clear();
        }
    }
}
