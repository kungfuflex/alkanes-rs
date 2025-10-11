use byteorder::{ByteOrder, LittleEndian};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EnvironmentInput {
    pub height: u32,
    pub data: Vec<u8>,
}

impl EnvironmentInput {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let height = LittleEndian::read_u32(&bytes[0..4]);
        let data = bytes[4..].to_vec();
        Self { height, data }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }
}

use downcast_rs::{impl_downcast, Downcast};

pub trait RuntimeEnvironment: std::fmt::Debug + Downcast {
    fn get(&mut self, key: &[u8]) -> Option<Vec<u8>>;
    fn flush(&mut self, data: &[u8]) -> Result<(), ()>;
    fn load_input(&self) -> Result<EnvironmentInput, ()>;
    fn log(&self, message: &str);
    fn clear(&mut self);
    fn cache(&mut self) -> &mut HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
        unimplemented!("cache not implemented for this environment")
    }
    fn to_flush(&mut self) -> &mut Vec<Arc<Vec<u8>>> {
        unimplemented!("to_flush not implemented for this environment")
    }
}

impl_downcast!(RuntimeEnvironment);
