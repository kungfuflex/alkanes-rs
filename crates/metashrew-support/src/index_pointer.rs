use crate::byte_view::ByteView;
use std::sync::Arc;

use hex;

pub trait KeyValuePointer {
    fn wrap(word: &Vec<u8>) -> Self;
    fn unwrap(&self) -> Arc<Vec<u8>>;
    fn set(&mut self, v: Arc<Vec<u8>>);
    fn _get(&self) -> Arc<Vec<u8>>;
    fn get(&self) -> Arc<Vec<u8>> {
      Arc::new(hex::decode(self._get().as_ref()).unwrap())
    }
    fn inherits(&mut self, from: &Self);
    fn _set(&mut self, v: Arc<Vec<u8>>) {
      self.set(Arc::new(hex::encode(v.as_ref()).as_bytes().to_vec()))
    }
    fn select(&self, word: &Vec<u8>) -> Self
    where
        Self: Sized,
    {
        let mut key = (*self.unwrap()).clone();
        key.extend(&hex::encode(word).as_bytes().to_vec());
//        key.extend(word);
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

    fn set_value<T: ByteView>(&mut self, v: T) {
        self._set(Arc::new(v.to_bytes()));
    }

    fn get_value<T: ByteView>(&self) -> T {
        let cloned = self.get().as_ref().clone();
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
    fn length(&self) -> u32
    where
        Self: Sized,
    {
        self.length_key().get_value::<u32>()
    }
    fn select_index(&self, index: u32) -> Self
    where
        Self: Sized,
    {
        self.keyword(&format!("/{}", index))
    }

    fn get_list(&self) -> Vec<Arc<Vec<u8>>>
    where
        Self: Sized,
    {
        let mut result: Vec<Arc<Vec<u8>>> = vec![];
        for i in 0..self.length() {
            result.push(self.select_index(i as u32).get().clone());
        }
        result
    }
    fn get_list_values<T: ByteView>(&self) -> Vec<T>
    where
        Self: Sized,
    {
        let mut result: Vec<T> = vec![];
        for i in 0..self.length() {
            result.push(self.select_index(i as u32).get_value());
        }
        result
    }
    fn nullify(&mut self) {
        self._set(Arc::from(vec![0]))
    }
    fn set_or_nullify(&mut self, v: Arc<Vec<u8>>) {
        let val = Arc::try_unwrap(v).unwrap();
        if <usize>::from_bytes(val.clone()) == 0 {
            self.nullify();
        } else {
            self._set(Arc::from(val));
        }
    }

    fn pop(&self) -> Arc<Vec<u8>>
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>();

        if length == 0 {
            return Arc::new(Vec::new()); // Return empty Vec if there are no elements
        }

        let new_length = length - 1;
        length_key.set_value::<u32>(new_length); // Update the length
        self.select_index(new_length).get() // Return the value at the new length
    }

    fn pop_value<T: ByteView>(&self) -> T
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>();

        if length == 0 {
            return T::from_bytes(Vec::new()); // Return a default value if there are no elements
        }

        let new_length = length - 1;
        length_key.set_value::<u32>(new_length); // Update the length
        self.select_index(new_length).get_value::<T>() // Return the value at the new length
    }

    fn append(&self, v: Arc<Vec<u8>>)
    where
        Self: Sized,
    {
        let mut new_index = self.extend();
        new_index._set(v);
    }

    fn append_value<T: ByteView>(&self, v: T)
    where
        Self: Sized,
    {
        let mut new_index = self.extend();
        new_index.set_value(v);
    }

    fn extend(&self) -> Self
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>();
        length_key.set_value::<u32>(length + 1);
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
}
