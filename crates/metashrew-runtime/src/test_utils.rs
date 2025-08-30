//! In-memory implementation of KeyValueStoreLike trait for fast testing

use crate::{to_labeled_key, BatchLike, KeyValueStoreLike};
use std::collections::HashMap;
use std::io::{Error, Result};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct MemStoreAdapter {
    pub db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    pub height: u32,
}

impl MemStoreAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone)]
pub struct MemStoreBatch {
    operations: Vec<BatchOperation>,
}

impl Default for MemStoreBatch {
    fn default() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

#[derive(Clone)]
enum BatchOperation {
    Put(Vec<u8>, Vec<u8>),
    Delete(Vec<u8>),
}

impl BatchLike for MemStoreBatch {
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) {
        self.operations.push(BatchOperation::Put(
            key.as_ref().to_vec(),
            value.as_ref().to_vec(),
        ));
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) {
        self.operations
            .push(BatchOperation::Delete(key.as_ref().to_vec()));
    }

    fn default() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

impl KeyValueStoreLike for MemStoreAdapter {
    type Batch = MemStoreBatch;
    type Error = Error;

    fn write(&mut self, batch: Self::Batch) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        for operation in batch.operations {
            match operation {
                BatchOperation::Put(key, value) => {
                    db.insert(to_labeled_key(&key), value);
                }
                BatchOperation::Delete(key) => {
                    db.remove(&to_labeled_key(&key));
                }
            }
        }
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();
        Ok(db.get(&to_labeled_key(&key.as_ref().to_vec())).cloned())
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();
        Ok(db.get(&to_labeled_key(&key.as_ref().to_vec())).cloned())
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let mut db = self.db.lock().unwrap();
        db.insert(
            to_labeled_key(&key.as_ref().to_vec()),
            value.as_ref().to_vec(),
        );
        Ok(())
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<()> {
        let mut db = self.db.lock().unwrap();
        db.remove(&to_labeled_key(&key.as_ref().to_vec()));
        Ok(())
    }

    fn scan_prefix<K: AsRef<[u8]>>(&self, prefix: K) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let db = self.db.lock().unwrap();
        let prefix_bytes = to_labeled_key(&prefix.as_ref().to_vec());
        let mut results = Vec::new();
        for (key, value) in db.iter() {
            if key.starts_with(&prefix_bytes) {
                results.push((key.clone(), value.clone()));
            }
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    fn create_batch(&self) -> Self::Batch {
        <MemStoreBatch as Default>::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>> {
        let db = self.db.lock().unwrap();
        let keys = db.keys().cloned().collect::<Vec<Vec<u8>>>();
        Ok(Box::new(keys.into_iter()))
    }

    fn clear(&mut self) -> Result<()> {
        self.db.lock().unwrap().clear();
        self.height = 0;
        Ok(())
    }
}