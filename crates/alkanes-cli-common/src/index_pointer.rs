pub use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct StubPointer;

impl KeyValuePointer for StubPointer {
    fn wrap(_word: &Vec<u8>) -> Self {
        Self
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        Arc::new(vec![])
    }
    fn inherits(&mut self, _v: &Self) {}
    fn set(&mut self, _v: Arc<Vec<u8>>) {
        // no-op
    }
    fn get(&self) -> Arc<Vec<u8>> {
        Arc::new(vec![])
    }
}
