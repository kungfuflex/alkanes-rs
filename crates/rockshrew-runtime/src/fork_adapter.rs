use crate::adapter::RocksDBBatch;
use crate::RocksDBRuntimeAdapter;
use anyhow::Result;
use metashrew_runtime::{BatchLike, KeyValueStoreLike};
use rocksdb::{DB, WriteBatchIterator};
use std::sync::Arc;

#[derive(Clone)]
pub enum ForkAdapter {
    Modern(RocksDBRuntimeAdapter),
    Legacy(LegacyRocksDBRuntimeAdapter),
}

impl KeyValueStoreLike for ForkAdapter {
    type Batch = crate::adapter::RocksDBBatch;
    type Error = rocksdb::Error;

    fn track_kv_update(&mut self, key: Vec<u8>, value: Vec<u8>) {
        match self {
            ForkAdapter::Modern(adapter) => adapter.track_kv_update(key, value),
            ForkAdapter::Legacy(adapter) => adapter.track_kv_update(key, value),
        }
    }

    fn write(&mut self, batch: crate::adapter::RocksDBBatch) -> Result<(), Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.write(batch),
            ForkAdapter::Legacy(adapter) => {
                let mut legacy_batch = crate::adapter::RocksDBBatch::default();
                struct LegacyBatchBuilder<'a> {
                    legacy_batch: &'a mut crate::adapter::RocksDBBatch,
                }
                impl<'a> WriteBatchIterator for LegacyBatchBuilder<'a> {
                    fn put(&mut self, key: Box<[u8]>, value: Box<[u8]>) {
                        self.legacy_batch.put(&*key, &*value);
                    }
                    fn delete(&mut self, key: Box<[u8]>) {
                        self.legacy_batch.delete(&*key);
                    }
                }
                batch.0.iterate(&mut LegacyBatchBuilder {
                    legacy_batch: &mut legacy_batch,
                });
                adapter.write(legacy_batch)
            }
        }
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.get(key),
            ForkAdapter::Legacy(adapter) => adapter.get(key),
        }
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.get_immutable(key),
            ForkAdapter::Legacy(adapter) => adapter.get_immutable(key),
        }
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.delete(key),
            ForkAdapter::Legacy(adapter) => adapter.delete(key),
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Result<(), Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.put(key, value),
            ForkAdapter::Legacy(adapter) => adapter.put(key, value),
        }
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.scan_prefix(prefix),
            ForkAdapter::Legacy(adapter) => adapter.scan_prefix(prefix),
        }
    }

    fn create_batch(&self) -> Self::Batch {
        match self {
            ForkAdapter::Modern(adapter) => adapter.create_batch(),
            ForkAdapter::Legacy(_adapter) => crate::adapter::RocksDBBatch(rocksdb::WriteBatch::default()),
        }
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        match self {
            ForkAdapter::Modern(adapter) => adapter.keys(),
            ForkAdapter::Legacy(adapter) => adapter.keys(),
        }
    }

    fn is_open(&self) -> bool {
        match self {
            ForkAdapter::Modern(adapter) => adapter.is_open(),
            ForkAdapter::Legacy(adapter) => adapter.is_open(),
        }
    }

    fn set_height(&mut self, height: u32) {
        match self {
            ForkAdapter::Modern(adapter) => adapter.set_height(height),
            ForkAdapter::Legacy(adapter) => adapter.set_height(height),
        }
    }

    fn get_height(&self) -> u32 {
        match self {
            ForkAdapter::Modern(adapter) => adapter.get_height(),
            ForkAdapter::Legacy(adapter) => adapter.get_height(),
        }
    }
}

#[derive(Clone)]
pub struct LegacyRocksDBRuntimeAdapter {
    pub db: Arc<DB>,
    pub fork_db: Option<Arc<DB>>,
    pub height: u32,
    pub kv_tracker: Arc<std::sync::Mutex<Option<metashrew_runtime::KVTrackerFn>>>,
}

impl LegacyRocksDBRuntimeAdapter {
    pub fn to_labeled_key(key: &Vec<u8>) -> Vec<u8> {
        if metashrew_runtime::has_label() {
            let mut result: Vec<u8> = vec![];
            result.extend(metashrew_runtime::get_label().as_str().as_bytes());
            result.extend(key);
            result
        } else {
            key.clone()
        }
    }
}


impl KeyValueStoreLike for LegacyRocksDBRuntimeAdapter {
    type Batch = crate::adapter::RocksDBBatch;
    type Error = rocksdb::Error;

    fn track_kv_update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {}

    fn write(&mut self, batch: crate::adapter::RocksDBBatch) -> Result<(), Self::Error> {
        self.db.write(batch.0)
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let labeled_key = LegacyRocksDBRuntimeAdapter::to_labeled_key(&key.as_ref().to_vec());
        match self.db.get(&labeled_key)? {
            Some(value) => Ok(Some(value.to_vec())),
            None => {
                if let Some(fork_db) = &self.fork_db {
                    fork_db.get(labeled_key).map(|opt| opt.map(|v| v.to_vec()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let labeled_key = LegacyRocksDBRuntimeAdapter::to_labeled_key(&key.as_ref().to_vec());
        match self.db.get(&labeled_key)? {
            Some(value) => Ok(Some(value.to_vec())),
            None => {
                if let Some(fork_db) = &self.fork_db {
                    fork_db.get(labeled_key).map(|opt| opt.map(|v| v.to_vec()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        self.db
            .delete(LegacyRocksDBRuntimeAdapter::to_labeled_key(&key.as_ref().to_vec()))
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Result<(), Self::Error> {
        self.db.put(
            LegacyRocksDBRuntimeAdapter::to_labeled_key(&key.as_ref().to_vec()),
            value,
        )
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        let mut results = Vec::new();
        let prefix_bytes =
            LegacyRocksDBRuntimeAdapter::to_labeled_key(&prefix.as_ref().to_vec());
        let mut iter = self.db.raw_iterator();
        iter.seek(&prefix_bytes);
        while iter.valid() {
            if let Some(key) = iter.key() {
                if !key.starts_with(&prefix_bytes) {
                    break;
                }
                if let Some(value) = iter.value() {
                    results.push((key.to_vec(), value.to_vec()));
                }
            }
            iter.next();
        }
        Ok(results)
    }

    fn create_batch(&self) -> Self::Batch {
        RocksDBBatch::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        Ok(Box::new(iter.map(|item| {
            let (key, _) = item.unwrap();
            key.to_vec()
        })))
    }

    fn is_open(&self) -> bool {
        true
    }

    fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    fn get_height(&self) -> u32 {
        self.height
    }
}