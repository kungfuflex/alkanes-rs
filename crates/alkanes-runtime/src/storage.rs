use crate::runtime::AlkaneResponder;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

struct StorageHandle(());

impl AlkaneResponder for StorageHandle {}

const RUNTIME_STORAGE: StorageHandle = StorageHandle(());

#[derive(Debug, Clone, Default)]
pub struct StoragePointer(pub Arc<Vec<u8>>);

impl StoragePointer {
    pub fn from_keyword(keyword: &str) -> Self {
        StoragePointer(Arc::new(keyword.as_bytes().to_vec()))
    }
}

#[allow(dead_code)]
impl KeyValuePointer for StoragePointer {
    fn wrap(word: &Vec<u8>) -> StoragePointer {
        StoragePointer(Arc::<Vec<u8>>::new(word.clone()))
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        self.0.clone()
    }
    fn inherits(&mut self, _v: &Self) {}
    fn set(&mut self, v: Arc<Vec<u8>>) {
        RUNTIME_STORAGE.store(self.unwrap().as_ref().clone(), v.as_ref().clone())
    }
    fn get(&self) -> Arc<Vec<u8>> {
        Arc::new(RUNTIME_STORAGE.load(self.unwrap().as_ref().clone()))
    }
}
