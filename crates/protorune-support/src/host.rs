use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;

pub trait Host {
    type Pointer: KeyValuePointer;
    fn get(&self, key: &[u8]) -> Result<Vec<u8>>;
    fn flush(&self);
    fn println(&self, message: &str);
}