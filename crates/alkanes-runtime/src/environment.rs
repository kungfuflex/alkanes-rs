use crate::imports::{__height, __load_context, __log, __request_context, __request_storage, __load_storage};
use anyhow::Result;
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr},
    environment::{EnvironmentInput, RuntimeEnvironment},
};
use std::{
    collections::HashMap,
    sync::Arc,
};

#[derive(Debug, Clone, Default)]
pub struct AlkaneEnvironment {
    pub cache: HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>,
    pub to_flush: Vec<Arc<Vec<u8>>>,
}

impl AlkaneEnvironment {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RuntimeEnvironment for AlkaneEnvironment {
    fn get(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let mut key_bytes = to_arraybuffer_layout(key);
        let key_ptr = to_passback_ptr(&mut key_bytes);
        let buf_size = unsafe { __request_storage(key_ptr) as usize };
        if buf_size == 0 {
            return None;
        }
        let mut buffer: Vec<u8> = to_arraybuffer_layout(&vec![0; buf_size]);
        let buffer_ptr = to_passback_ptr(&mut buffer);
        unsafe {
            __load_storage(key_ptr, buffer_ptr);
        }
        Some((&buffer[4..]).to_vec())
    }

    fn flush(&mut self, _data: &[u8]) -> Result<(), ()> {
        unimplemented!("flush not implemented for this")
    }

    fn load_input(&self) -> Result<EnvironmentInput, ()> {
        unsafe {
            let height = {
                let mut buffer: Vec<u8> = to_arraybuffer_layout(&vec![0; 8]);
                __height(to_ptr(&mut buffer) + 4);
                u64::from_le_bytes((&buffer[4..]).try_into().unwrap()) as u32
            };
            let data = {
                let mut buffer: Vec<u8> =
                    to_arraybuffer_layout(&vec![0; __request_context() as usize]);
                __load_context(to_ptr(&mut buffer) + 4);
                (&buffer[4..]).to_vec()
            };
            Ok(EnvironmentInput { height, data })
        }
    }

    fn log(&self, message: &str) {
        let mut buffer = to_arraybuffer_layout(message.as_bytes());
        let ptr = to_passback_ptr(&mut buffer);
        unsafe {
            __log(ptr);
        }
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.to_flush.clear();
    }

    fn cache(&mut self) -> &mut HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
        &mut self.cache
    }

    fn to_flush(&mut self) -> &mut Vec<Arc<Vec<u8>>> {
        &mut self.to_flush
    }
}