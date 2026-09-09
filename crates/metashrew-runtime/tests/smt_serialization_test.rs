use metashrew_runtime::smt::BatchedSMTHelper;
use metashrew_runtime::traits::{BatchLike, KeyValueStoreLike};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::fmt;

#[derive(Debug)]
struct MockError(String);

impl fmt::Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MockError {}

#[derive(Clone)]
struct MockBatch {
    operations: Arc<Mutex<Vec<(Vec<u8>, Vec<u8>)>>>,
}

impl BatchLike for MockBatch {
    fn default() -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, k: K, v: V) {
        let key = k.as_ref().to_vec();
        let value = v.as_ref().to_vec();
        self.operations.lock().unwrap().push((key, value));
    }

    fn delete<K: AsRef<[u8]>>(&mut self, _k: K) {}
}

#[derive(Clone)]
struct MockStorage {
    data: Arc<Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn get_raw(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.lock().unwrap().get(key).cloned()
    }
}

impl KeyValueStoreLike for MockStorage {
    type Batch = MockBatch;
    type Error = MockError;

    fn write(&mut self, batch: Self::Batch) -> Result<(), Self::Error> {
        let ops = batch.operations.lock().unwrap();
        let mut data = self.data.lock().unwrap();
        for (key, value) in ops.iter() {
            data.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.data.lock().unwrap().get(key.as_ref()).cloned())
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.data.lock().unwrap().get(key.as_ref()).cloned())
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        self.data.lock().unwrap().remove(key.as_ref());
        Ok(())
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Result<(), Self::Error> {
        self.data
            .lock()
            .unwrap()
            .insert(key.as_ref().to_vec(), value.as_ref().to_vec());
        Ok(())
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        let prefix_bytes = prefix.as_ref();
        Ok(self
            .data
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, _)| k.starts_with(prefix_bytes))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn create_batch(&self) -> Self::Batch {
        MockBatch::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        let keys: Vec<Vec<u8>> = self.data.lock().unwrap().keys().cloned().collect();
        Ok(Box::new(keys.into_iter()))
    }
}

#[test]
fn test_tip_height_serialization() {
    use metashrew_runtime::runtime::TIP_HEIGHT_KEY;

    let storage = MockStorage::new();

    // Simulate what happens in calculate_and_store_state_root_batched
    let height: u32 = 1;
    let mut batch = storage.create_batch();

    // This is the problematic line from smt.rs:403
    batch.put(
        &TIP_HEIGHT_KEY.as_bytes().to_vec(),
        &height.to_le_bytes(),
    );

    // Check what was actually put in the batch
    let ops = batch.operations.lock().unwrap();
    assert_eq!(ops.len(), 1);
    let (key, value) = &ops[0];

    println!("Key: {:?}", String::from_utf8_lossy(key));
    println!("Value bytes: {:?}", value);
    println!("Value len: {}", value.len());
    println!("Value as u32 (LE): {}", u32::from_le_bytes(value[..4].try_into().unwrap()));

    // The value should be [1, 0, 0, 0] for height=1 in little-endian
    assert_eq!(value.len(), 4, "tip-height value should be 4 bytes");
    assert_eq!(value, &vec![1, 0, 0, 0], "tip-height should be [1, 0, 0, 0] for height=1");
}

#[test]
fn test_tip_height_storage_and_retrieval() {
    use metashrew_runtime::runtime::TIP_HEIGHT_KEY;

    let mut storage = MockStorage::new();

    // Write height=1
    let height: u32 = 1;
    storage.put(
        TIP_HEIGHT_KEY.as_bytes(),
        &height.to_le_bytes(),
    ).unwrap();

    // Read it back
    let stored_value = storage.get_raw(TIP_HEIGHT_KEY.as_bytes())
        .expect("tip-height should be stored");

    println!("Stored value: {:?}", stored_value);
    println!("Stored value len: {}", stored_value.len());

    assert_eq!(stored_value.len(), 4, "stored tip-height should be 4 bytes");

    let retrieved_height = u32::from_le_bytes(stored_value[..4].try_into().unwrap());
    println!("Retrieved height: {}", retrieved_height);

    assert_eq!(retrieved_height, 1, "retrieved height should be 1");
}
