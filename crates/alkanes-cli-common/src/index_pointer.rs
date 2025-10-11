pub use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;
use metashrew_support::environment::RuntimeEnvironment;

#[derive(Clone, Debug, Default)]
pub struct StubPointer;

impl<E: RuntimeEnvironment> KeyValuePointer<E> for StubPointer {
    fn wrap(_word: &Vec<u8>) -> Self {
        Self
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        Arc::new(vec![])
    }
    fn inherits(&mut self, _v: &Self) {}
    fn set(&mut self, _env: &mut E, _v: Arc<Vec<u8>>) {
        // no-op
    }
    fn get(&self, _env: &mut E) -> Arc<Vec<u8>> {
        Arc::new(vec![])
    }
}