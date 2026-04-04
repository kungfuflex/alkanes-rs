//! KeyValuePointer — generic hierarchical KV storage interface.
//!
//! Mirrors metashrew-support's KeyValuePointer trait. Implementations
//! provide `wrap`, `unwrap`, `get`, `set`, `inherits` — all list/tree
//! operations are derived from those five primitives.

use std::sync::Arc;
use crate::byte_view::ByteView;

/// A pointer into a hierarchical key-value store.
///
/// Implementors define how `get`/`set` resolve — may hit local storage,
/// secondary indexer storage, or an in-memory checkpoint stack.
pub trait KeyValuePointer: Clone {
    /// Create a pointer from raw key bytes.
    fn wrap(word: &Vec<u8>) -> Self;
    /// Extract the raw key bytes.
    fn unwrap(&self) -> Arc<Vec<u8>>;
    /// Store raw bytes at this key.
    fn set(&mut self, v: Arc<Vec<u8>>);
    /// Retrieve raw bytes from this key.
    fn get(&self) -> Arc<Vec<u8>>;
    /// Inherit configuration (e.g., checkpoint stack) from a parent pointer.
    fn inherits(&mut self, from: &Self);

    // -- Key composition --------------------------------------------------

    /// Extend key with raw bytes.
    fn select(&self, word: &Vec<u8>) -> Self {
        let mut key = (*self.unwrap()).clone();
        key.extend(word);
        let mut ptr = Self::wrap(&key);
        ptr.inherits(self);
        ptr
    }

    /// Create a new pointer from a string keyword.
    fn from_keyword(word: &str) -> Self {
        Self::wrap(&word.as_bytes().to_vec())
    }

    /// Extend key with a string keyword.
    fn keyword(&self, word: &str) -> Self {
        let mut key = (*self.unwrap()).clone();
        key.extend(word.as_bytes());
        let mut ptr = Self::wrap(&key);
        ptr.inherits(self);
        ptr
    }

    /// Prepend a keyword to the key.
    fn prefix(&self, word: &str) -> Self {
        let mut key = word.as_bytes().to_vec();
        key.extend(self.unwrap().iter());
        let mut ptr = Self::wrap(&key);
        ptr.inherits(self);
        ptr
    }

    // -- Typed values -----------------------------------------------------

    /// Store a typed value (serialized via ByteView).
    fn set_value<T: ByteView>(&mut self, v: T) {
        self.set(Arc::new(v.to_bytes()));
    }

    /// Retrieve a typed value (deserialized via ByteView).
    fn get_value<T: ByteView>(&self) -> T {
        T::from_bytes((*self.get()).clone())
    }

    /// Create a child pointer with a typed key suffix.
    fn select_value<T: ByteView>(&self, key: T) -> Self {
        self.select(key.to_bytes().as_ref())
    }

    /// Set value to zero.
    fn nullify(&mut self) {
        self.set(Arc::new(vec![0]));
    }

    // -- List operations --------------------------------------------------
    // Lists use `{key} ++ u32::MAX_LE` for length, `{key} ++ index_LE` for items.

    /// Pointer to the list length metadata.
    fn length_key(&self) -> Self {
        self.select_value::<u32>(u32::MAX)
    }

    /// Pointer to a list element by index.
    fn select_index(&self, index: u32) -> Self {
        self.select_value::<u32>(index)
    }

    /// Get the current list length.
    fn length(&self) -> u32 {
        self.length_key().get_value::<u32>()
    }

    /// Append raw bytes to end of list.
    fn append(&self, v: Arc<Vec<u8>>) {
        let mut len_ptr = self.length_key();
        let len = len_ptr.get_value::<u32>();
        let mut item_ptr = self.select_index(len);
        item_ptr.set(v);
        len_ptr.set_value::<u32>(len + 1);
    }

    /// Append a typed value to end of list.
    fn append_value<T: ByteView>(&self, v: T) {
        self.append(Arc::new(v.to_bytes()));
    }

    /// Get all list items as raw bytes.
    fn get_list(&self) -> Vec<Arc<Vec<u8>>> {
        let len = self.length();
        (0..len).map(|i| self.select_index(i).get()).collect()
    }

    /// Get all list items as typed values.
    fn get_list_values<T: ByteView>(&self) -> Vec<T> {
        let len = self.length();
        (0..len).map(|i| self.select_index(i).get_value::<T>()).collect()
    }

    /// Remove and return the last item.
    fn pop(&self) -> Arc<Vec<u8>> {
        let mut len_ptr = self.length_key();
        let len = len_ptr.get_value::<u32>();
        if len == 0 { return Arc::new(vec![]); }
        let item = self.select_index(len - 1).get();
        len_ptr.set_value::<u32>(len - 1);
        item
    }
}
