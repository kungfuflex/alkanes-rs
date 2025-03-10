use bitcoin::sighash::Prevouts;

use crate::byte_view::ByteView;
use core::prelude;
use std::sync::Arc;

pub trait KeyValuePointer {
    fn wrap(word: &Vec<u8>) -> Self;
    fn unwrap(&self) -> Arc<Vec<u8>>;
    fn set(&mut self, v: Arc<Vec<u8>>);
    fn get(&self) -> Arc<Vec<u8>>;
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

    fn set_value<T: ByteView>(&mut self, v: T) {
        self.set(Arc::new(v.to_bytes()));
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

    fn drop_index(&self, index: u32) -> ()
    where
        Self: Sized,
    {
        let mut idx = self.keyword(&format!("/{}", index));
        idx.nullify();
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
        self.set(Arc::from(vec![0]))
    }
    fn set_or_nullify(&mut self, v: Arc<Vec<u8>>) {
        let val = Arc::try_unwrap(v).unwrap();
        if <usize>::from_bytes(val.clone()) == 0 {
            self.nullify();
        } else {
            self.set(Arc::from(val));
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
        new_index.set(v);
    }
    fn append_ll(&self, v: Arc<Vec<u8>>)
    where
        Self: Sized,
    {
        let mut new_index = self.extend_ll();
        new_index.set(v);
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
    fn extend_ll(&self) -> Self
    where
        Self: Sized,
    {
        let mut length_key = self.length_key();
        let length = length_key.get_value::<u32>();
        if length > 0 {
            let mut next_key = self.next_key(length - 1);
            next_key.set_value(length);
        }
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
    fn set_next_for(&self, i: u32, v: u32) -> ()
    where
        Self: Sized,
    {
        let mut next_key = self.next_key(i);
        next_key.set_value(v);
    }
    fn delete_value(&self, i: u32) -> ()
    where
        Self: Sized,
    {
        let mut head_key = self.head_key();
        if i == head_key.get_value() {
            let next = self.next_key(i).get_value::<u32>();
            head_key.set_value(next);
        } else {
            let mut prev = self.next_key(i - 1);
            let next = self.next_key(i).get_value::<u32>();
            prev.set_value(next);
        }
        self.drop_index(i);
    }
    fn map_ll<T>(&self, mut f: impl FnMut(&mut Self, u32) -> T) -> Vec<T>
    where
        Self: Sized + Clone,
    {
        let length_key = self.length_key();
        let length = length_key.get_value::<u32>();
        let mut result = Vec::new();
        let mut i: u32 = 0;
        while i < length {
            let item = self.select_index(i);
            let mut item_mut = item.clone();
            result.push(f(&mut item_mut, i));
            i = self.next_key(i).get_value::<u32>();
            if i == 0 {
                break;
            }
        }
        result
    }
}
