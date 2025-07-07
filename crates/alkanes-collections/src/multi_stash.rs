//! Multi-stash implementation for managing multiple stacks with keys
//! 
//! This module provides a MultiStash-like data structure that can manage
//! multiple stacks of values with associated keys and quantities.

use alkanes_alloc::AlkanesAllocator;
use crate::{Map, AlkanesVec};
use core::{
    fmt,
    num::NonZeroUsize,
};

/// A key type for identifying stash entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(usize);

impl From<usize> for Key {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<Key> for usize {
    fn from(key: Key) -> Self {
        key.0
    }
}

/// A multi-stash data structure that manages multiple stacks with keys
pub struct AlkanesMultiStash<T, A: AlkanesAllocator> {
    /// Map from keys to (quantity, value) pairs
    entries: Map<Key, (usize, T)>,
    /// Next available key
    next_key: usize,
    /// Allocator for internal data structures
    allocator: A,
}

impl<T, A: AlkanesAllocator> AlkanesMultiStash<T, A> {
    /// Creates a new empty multi-stash
    pub fn new(allocator: A) -> Self {
        Self {
            entries: Map::new(),
            next_key: 0,
            allocator,
        }
    }

    /// Puts a value with the given quantity and returns a key
    pub fn put(&mut self, quantity: NonZeroUsize, value: T) -> Key {
        let key = Key(self.next_key);
        self.next_key += 1;
        self.entries.insert(key, (quantity.get(), value));
        key
    }

    /// Gets the entry for the given key
    pub fn get(&self, key: Key) -> Option<(usize, &T)> {
        self.entries.get(&key).map(|(qty, val)| (*qty, val))
    }

    /// Bumps the quantity for the given key by the specified amount
    /// Returns the old quantity if the key exists
    pub fn bump(&mut self, key: Key, amount: usize) -> Option<usize> {
        if let Some((qty, _)) = self.entries.get_mut(&key) {
            let old_qty = *qty;
            *qty += amount;
            Some(old_qty)
        } else {
            None
        }
    }

    /// Takes one item from the given key's quantity
    /// Returns the value if the quantity reaches zero
    pub fn take_one(&mut self, key: Key) -> Option<T> {
        if let Some((qty, _)) = self.entries.get_mut(&key) {
            if *qty > 1 {
                *qty -= 1;
                None
            } else {
                // Quantity is 1, remove the entry entirely
                self.entries.remove(&key).map(|(_, value)| value)
            }
        } else {
            None
        }
    }

    /// Takes all items for the given key
    /// Returns the value regardless of quantity
    pub fn take_all(&mut self, key: Key) -> Option<T> {
        self.entries.remove(&key).map(|(_, value)| value)
    }

    /// Clears all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.next_key = 0;
    }

    /// Returns true if the multi-stash is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of unique keys
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl<T, A: AlkanesAllocator> Default for AlkanesMultiStash<T, A>
where
    A: Default,
{
    fn default() -> Self {
        Self::new(A::default())
    }
}

impl<T, A: AlkanesAllocator> fmt::Debug for AlkanesMultiStash<T, A>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlkanesMultiStash")
            .field("entries", &self.entries)
            .field("next_key", &self.next_key)
            .finish()
    }
}

// Type alias for convenience with default allocator
use alkanes_alloc::DefaultAllocator;
pub type MultiStash<T> = AlkanesMultiStash<T, DefaultAllocator>;