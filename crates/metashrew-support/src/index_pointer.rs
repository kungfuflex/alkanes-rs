
use crate::byte_view::ByteView;
use crate::cache::{get, set};
use crate::environment::RuntimeEnvironment;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

pub trait KeyValuePointer<E: RuntimeEnvironment> {
    fn wrap(word: &Vec<u8>) -> Self;
    fn unwrap(&self) -> Arc<Vec<u8>>;
    fn set(&mut self, env: &mut E, v: Arc<Vec<u8>>);
    fn get(&self, env: &mut E) -> Arc<Vec<u8>>;
    fn inherits(&mut self, from: &Self);
    fn select(&self, word: &Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let mut key = (*self.unwrap()).clone();
        key.extend(word);
        let mut ptr = Self::wrap(&key);
        ptr.inherits(self);
        ptr
    }
    fn from_keyword(word: &str) -> Self
    where
        Self: Sized,
    {
        Self::wrap(&word.as_bytes().to_vec())
    }
    fn keyword(&self, word: &str) -> Self
    where
        Self: Sized,
    {
        let mut key = (*self.unwrap()).clone();
        key.extend(word.to_string().into_bytes());
        let mut ptr = Self::wrap(&key);
        ptr.inherits(self);
        ptr
    }

    fn set_value<T: ByteView>(&mut self, env: &mut E, v: T) {
        self.set(env, Arc::new(v.to_bytes()));
    }

    fn get_value<T: ByteView>(&self, env: &mut E) -> T {
        let cloned = self.get(env).as_ref().clone();
        if cloned.is_empty() {
            T::zero()
        } else {
            T::from_bytes(cloned)
        }
    }

    fn select_value<T: ByteView>(&self, key: T) -> Self
    where
        Self: Sized,
    {
        self.select(key.to_bytes().as_ref())
    }
    fn length_key(&self) -> Self
    where
        Self: Sized,
    {
        self.keyword(&"/length".to_string())
    }
    fn head_key(&self) -> Self
    where
        Self: Sized,
    {
        self.keyword(&"/head".to_string())
    }
    fn next_key(&self, i: u32) -> Self
    where
        Self: Sized,
    {
        self.keyword(&"/next".to_string()).select_value(i)
    }
    fn length(&self, env: &mut E) -> u32
    where
        Self: Sized,
    {
        self.length_key().get_value::<u32>(env)
    }
    fn select_index(&self, index: u32) -> Self
    where
        Self: Sized,
    {
        self.keyword(&format!("/{}", index))
    }

    fn drop_index(&mut self, env: &mut E, index: u32) -> ()
    where
        Self: Sized,
    {
        let mut idx = self.keyword(&format!("/{}", index));
        idx.nullify(env);
    }
    fn get_list(&self, env: &mut E) -> Vec<Arc<Vec<u8>>>
    where
        Self: Sized,
    {
        let mut result: Vec<Arc<Vec<u8>>> = vec![];
        for i in 0..self.length(env) {
            result.push(self.select_index(i as u32).get(env).clone());
        }
        result
    }
    fn get_list_values<T: ByteView>(&self, env: &mut E) -> Vec<T>
    where
        Self: Sized,
    {
        let mut result: Vec<T> = vec![];
        for i in 0..self.length(env) {
            result.push(self.select_index(i as u32).get_value(env));
        }
        result
    }
    fn nullify(&mut self, env: &mut E) {
        self.set(env, Arc::from(vec![0]))
    }
    fn set_or_nullify(&mut self, env: &mut E, v: Arc<Vec<u8>>) {
        let val = Arc::try_unwrap(v).unwrap();
        if <usize>::from_bytes(val.clone()) == 0 {
            self.nullify(env);
        } else {
            self.set(env, Arc::from(val));
        }
    }

    fn pop(&mut self, env: &mut E) -> Arc<Vec<u8>>
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>(env);

        if length == 0 {
            return Arc::new(Vec::new()); // Return empty Vec if there are no elements
        }

        let new_length = length - 1;
        length_key.set_value::<u32>(env, new_length); // Update the length
        self.select_index(new_length).get(env) // Return the value at the new length
    }

    fn pop_value<T: ByteView>(&mut self, env: &mut E) -> T
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>(env);

        if length == 0 {
            return T::from_bytes(Vec::new()); // Return a default value if there are no elements
        }

        let new_length = length - 1;
        length_key.set_value::<u32>(env, new_length); // Update the length
        self.select_index(new_length).get_value::<T>(env) // Return the value at the new length
    }

    fn append(&mut self, env: &mut E, v: Arc<Vec<u8>>) 
    where
        Self: Sized,
    {
        let mut new_index = self.extend(env);
        new_index.set(env, v);
    }
    fn append_ll(&mut self, env: &mut E, v: Arc<Vec<u8>>) 
    where
        Self: Sized,
    {
        let mut new_index = self.extend_ll(env);
        new_index.set(env, v);
    }
    fn append_value<T: ByteView>(&mut self, env: &mut E, v: T)
    where
        Self: Sized,
    {
        let mut new_index = self.extend(env);
        new_index.set_value(env, v);
    }

    fn extend(&mut self, env: &mut E) -> Self
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>(env);
        length_key.set_value::<u32>(env, length + 1);
        self.select_index(length)
    }
    fn extend_ll(&mut self, env: &mut E) -> Self
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>(env);
        if length > 0 {
            let mut next_key = self.next_key(length - 1);
            next_key.set_value(env, length);
        }
        length_key.set_value::<u32>(env, length + 1);
        self.select_index(length)
    }
    fn prefix(&self, keyword: &str) -> Self
    where
        Self: Sized,
    {
        let mut val = keyword.to_string().into_bytes();
        val.extend((*self.unwrap()).clone());
        let mut ptr = Self::wrap(&val);
        ptr.inherits(self);
        ptr
    }
    fn set_next_for(&mut self, env: &mut E, i: u32, v: u32) -> ()
    where
        Self: Sized,
    {
        let mut next_key = self.next_key(i);
        next_key.set_value(env, v);
    }
    fn delete_value(&mut self, env: &mut E, i: u32) -> ()
    where
        Self: Sized,
    {
        let mut head_key = self.head_key();
        if i == head_key.get_value::<u32>(env) {
            let next = self.next_key(i).get_value::<u32>(env);
            head_key.set_value::<u32>(env, next);
        }
        else {
            let mut prev = self.next_key(i - 1);
            let next = self.next_key(i).get_value::<u32>(env);
            prev.set_value::<u32>(env, next);
        }
        self.drop_index(env, i);
    }
    fn map_ll<T>(&self, env: &mut E, mut f: impl FnMut(&mut Self, u32) -> T) -> Vec<T>
    where
        Self: Sized + Clone,
    {
        let length_key = self.length_key();
        let length = length_key.get_value::<u32>(env);
        let mut result = Vec::new();
        let mut i: u32 = self.head_key().get_value::<u32>(env);
        while i < length {
            let item = self.select_index(i);
            let mut item_mut = item.clone();
            result.push(f(&mut item_mut, i));
            i = self.next_key(i).get_value::<u32>(env);
            if i == 0 {
                break;
            }
        }
        result
    }
}

#[derive(Debug, Default)]
pub struct IndexPointer<E: RuntimeEnvironment> {
    key: Arc<Vec<u8>>,
    _phantom: PhantomData<E>,
}

impl<E: RuntimeEnvironment> KeyValuePointer<E> for IndexPointer<E> {
    fn wrap(word: &Vec<u8>) -> Self {
        Self {
            key: Arc::new(word.clone()),
            _phantom: PhantomData,
        }
    }
    fn unwrap(&self) -> Arc<Vec<u8>> {
        self.key.clone()
    }
    fn inherits(&mut self, _v: &Self) {}
    fn set(&mut self, env: &mut E, v: Arc<Vec<u8>>) {
        set(env, self.unwrap(), v)
    }
    fn get(&self, env: &mut E) -> Arc<Vec<u8>> {
        get(env, self.unwrap())
    }
}

impl<E: RuntimeEnvironment> Clone for IndexPointer<E> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            _phantom: PhantomData,
        }
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

impl IndexCheckpointStack {
    pub fn depth(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

#[derive(Clone, Debug)]
pub struct AtomicPointer<E: RuntimeEnvironment> {
    pointer: IndexPointer<E>,
    store: IndexCheckpointStack,
}

impl<E: RuntimeEnvironment> KeyValuePointer<E> for AtomicPointer<E> {
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
    fn set(&mut self, _env: &mut E, v: Arc<Vec<u8>>) {
        self.store
            .0
            .lock()
            .unwrap()
            .last_mut()
            .unwrap()
            .0
            .insert(self.unwrap(), v.clone());
    }
    fn get(&self, env: &mut E) -> Arc<Vec<u8>> {
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
            None => self.pointer.get(env),
        }
    }
}

impl<E: RuntimeEnvironment> Default for AtomicPointer<E> {
    fn default() -> Self {
        AtomicPointer {
            pointer: IndexPointer::wrap(&Vec::<u8>::new()),
            store: IndexCheckpointStack::default(),
        }
    }
}

impl<E: RuntimeEnvironment> AtomicPointer<E> {
    pub fn checkpoint(&mut self) {
        self.store
            .0
            .lock()
            .unwrap()
            .push(IndexCheckpoint::default());
    }
    pub fn commit(&mut self, env: &mut E) {
        let checkpoints = &mut self.store.0.lock().unwrap();
        if checkpoints.len() > 1 {
            checkpoints
                .pop()
                .unwrap()
                .pipe_to(checkpoints.last_mut().unwrap());
        } else if checkpoints.len() == 1 {
            checkpoints.last().unwrap().0.iter().for_each(|(k, v)| {
                set(env, k.clone(), v.clone());
            });
        } else {
            panic!("commit() called without checkpoints in memory");
        }
    }
    pub fn rollback(&mut self) {
        self.store.0.lock().unwrap().pop();
    }
    pub fn derive(&self, pointer: &IndexPointer<E>) -> Self {
        AtomicPointer {
            store: self.store.clone(),
            pointer: pointer.clone(),
        }
    }
    pub fn get_pointer(&self) -> IndexPointer<E> {
        return self.pointer.clone();
    }

    // Get the current depth of the checkpoint stack
    pub fn checkpoint_depth(&self) -> usize {
        self.store.depth()
    }
}
