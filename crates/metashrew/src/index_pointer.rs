use crate::{get, set};
use metashrew_support::index_pointer::KeyValuePointer;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct IndexPointer(Arc<Vec<u8>>);

#[allow(dead_code)]
impl KeyValuePointer for IndexPointer {
    fn wrap(word: &Vec<u8>) -> IndexPointer {
        IndexPointer(Arc::<Vec<u8>>::new(word.clone()))
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        self.0.clone()
    }
    fn inherits(&mut self, _v: &Self) {}
    fn set(&mut self, v: Arc<Vec<u8>>) {
        set(self.unwrap(), v)
    }
    fn get(&self) -> Arc<Vec<u8>> {
        get(self.unwrap())
    }
}

#[derive(Clone, Default, Debug)]
pub struct IndexCheckpoint(pub HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>);

impl IndexCheckpoint {
    fn pipe_to(&self, target: &mut IndexCheckpoint) {
        self.0.iter().for_each(|(k, v)| {
            target.0.insert(k.clone(), v.clone());
        });
    }
}

#[derive(Clone, Debug)]
pub struct IndexCheckpointStack(pub Arc<Mutex<Vec<IndexCheckpoint>>>);

impl Default for IndexCheckpointStack {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(vec![IndexCheckpoint::default()])))
    }
}

#[derive(Clone, Debug)]
pub struct AtomicPointer {
    pointer: IndexPointer,
    store: IndexCheckpointStack,
}

impl KeyValuePointer for AtomicPointer {
    fn wrap(word: &Vec<u8>) -> Self {
        AtomicPointer {
            pointer: IndexPointer::wrap(word),
            store: IndexCheckpointStack::default(),
        }
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        self.pointer.unwrap()
    }
    fn inherits(&mut self, from: &Self) {
        self.store = from.store.clone()
    }
    fn set(&mut self, v: Arc<Vec<u8>>) {
        self.store
            .0
            .lock()
            .unwrap()
            .last_mut()
            .unwrap()
            .0
            .insert(self.unwrap(), v.clone());
    }
    fn get(&self) -> Arc<Vec<u8>> {
        let unwrapped = self.unwrap();
        match self
            .store
            .0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|map| map.0.contains_key(&unwrapped))
        {
            Some(map) => map.0.get(&unwrapped).unwrap().clone(),
            None => self.pointer.get(),
        }
    }
}

impl Default for AtomicPointer {
    fn default() -> Self {
        AtomicPointer {
            pointer: IndexPointer::wrap(&Vec::<u8>::new()),
            store: IndexCheckpointStack::default(),
        }
    }
}

impl AtomicPointer {
    pub fn checkpoint(&mut self) {
        self.store
            .0
            .lock()
            .unwrap()
            .push(IndexCheckpoint::default());
    }
    pub fn commit(&mut self) {
        let checkpoints = &mut self.store.0.lock().unwrap();
        if checkpoints.len() > 1 {
            checkpoints
                .pop()
                .unwrap()
                .pipe_to(checkpoints.last_mut().unwrap());
        } else if checkpoints.len() == 1 {
            checkpoints.last().unwrap().0.iter().for_each(|(k, v)| {
                set(k.clone(), v.clone());
            });
        } else {
            panic!("commit() called without checkpoints in memory");
        }
    }
    pub fn rollback(&mut self) {
        self.store.0.lock().unwrap().pop();
    }
    pub fn derive(&self, pointer: &IndexPointer) -> Self {
        AtomicPointer {
            store: self.store.clone(),
            pointer: pointer.clone(),
        }
    }
    pub fn get_pointer(&self) -> IndexPointer {
        return self.pointer.clone();
    }
}
