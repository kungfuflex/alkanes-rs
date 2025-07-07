//! Generic vector implementation using AlkanesAllocator
//! 
//! This module provides a Vec-like data structure that can work with
//! different allocator backends through the AlkanesAllocator trait.

use alkanes_alloc::AlkanesAllocator;
use core::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
    ptr,
    slice,
};

/// A generic vector that uses AlkanesAllocator for memory management
pub struct AlkanesVec<T, A: AlkanesAllocator> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
    allocator: A,
    _marker: PhantomData<T>,
}

impl<T, A: AlkanesAllocator> AlkanesVec<T, A> {
    /// Creates a new empty vector with the given allocator
    pub fn new(allocator: A) -> Self {
        Self {
            ptr: core::ptr::null_mut(),
            len: 0,
            capacity: 0,
            allocator,
            _marker: PhantomData,
        }
    }

    /// Creates a new vector with the specified capacity
    pub fn with_capacity(capacity: usize, allocator: A) -> Result<Self, A::Error> {
        let mut vec = Self::new(allocator);
        if capacity > 0 {
            vec.reserve(capacity)?;
        }
        Ok(vec)
    }

    /// Returns the number of elements in the vector
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the vector is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the vector
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Reserves capacity for at least `additional` more elements
    pub fn reserve(&mut self, additional: usize) -> Result<(), A::Error> {
        let required_cap = self.len.saturating_add(additional);
        if required_cap <= self.capacity {
            return Ok(());
        }

        let new_cap = required_cap.max(self.capacity * 2).max(4);
        self.grow(new_cap)
    }

    /// Grows the vector to the specified capacity
    fn grow(&mut self, new_cap: usize) -> Result<(), A::Error> {
        let new_size = new_cap * core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        
        let new_ptr = if self.capacity == 0 {
            // First allocation
            self.allocator.allocate(new_size, align)? as *mut T
        } else {
            // Reallocation
            let old_size = self.capacity * core::mem::size_of::<T>();
            let old_ptr = self.ptr as *mut u8;
            unsafe {
                self.allocator.reallocate(old_ptr, old_size, new_size, align)? as *mut T
            }
        };

        self.ptr = new_ptr;
        self.capacity = new_cap;
        Ok(())
    }

    /// Pushes an element to the end of the vector
    pub fn push(&mut self, value: T) -> Result<(), A::Error> {
        if self.len == self.capacity {
            self.reserve(1)?;
        }

        unsafe {
            ptr::write(self.ptr.add(self.len), value);
        }
        self.len += 1;
        Ok(())
    }

    /// Pops an element from the end of the vector
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                Some(ptr::read(self.ptr.add(self.len)))
            }
        }
    }

    /// Clears all elements from the vector
    pub fn clear(&mut self) {
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.ptr, self.len));
        }
        self.len = 0;
    }

    /// Returns a slice containing all elements
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.ptr, self.len)
        }
    }

    /// Returns a mutable slice containing all elements
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.ptr, self.len)
        }
    }

    /// Returns an iterator over the elements
    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Returns a mutable iterator over the elements
    #[inline]
    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    /// Extends the vector with elements from an iterator
    pub fn extend<I>(&mut self, iter: I) -> Result<(), A::Error>
    where
        I: IntoIterator<Item = T>,
    {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        self.reserve(lower)?;

        for item in iter {
            self.push(item)?;
        }
        Ok(())
    }

    /// Returns a reference to the element at the given index
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            unsafe {
                Some(&*self.ptr.add(index))
            }
        } else {
            None
        }
    }

    /// Returns a mutable reference to the element at the given index
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            unsafe {
                Some(&mut *self.ptr.add(index))
            }
        } else {
            None
        }
    }
}

impl<T, A: AlkanesAllocator> Drop for AlkanesVec<T, A> {
    fn drop(&mut self) {
        self.clear();
        
        if self.capacity > 0 && !self.ptr.is_null() {
            let size = self.capacity * core::mem::size_of::<T>();
            let align = core::mem::align_of::<T>();
            unsafe {
                self.allocator.deallocate(self.ptr as *mut u8, size, align);
            }
        }
    }
}

impl<T, A: AlkanesAllocator> Deref for AlkanesVec<T, A> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, A: AlkanesAllocator> DerefMut for AlkanesVec<T, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, A: AlkanesAllocator> Index<usize> for AlkanesVec<T, A> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T, A: AlkanesAllocator> IndexMut<usize> for AlkanesVec<T, A> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<T, A: AlkanesAllocator> fmt::Debug for AlkanesVec<T, A>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T, A: AlkanesAllocator> PartialEq for AlkanesVec<T, A>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T, A: AlkanesAllocator> Eq for AlkanesVec<T, A> where T: Eq {}

impl<T, A: AlkanesAllocator> Clone for AlkanesVec<T, A>
where
    T: Clone,
    A: Clone,
{
    fn clone(&self) -> Self {
        let mut new_vec = Self::new(self.allocator.clone());
        if let Ok(()) = new_vec.reserve(self.len()) {
            for item in self.iter() {
                let _ = new_vec.push(item.clone());
            }
        }
        new_vec
    }
}

impl<T, A: AlkanesAllocator> PartialOrd for AlkanesVec<T, A>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T, A: AlkanesAllocator> Ord for AlkanesVec<T, A>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T, A: AlkanesAllocator> FromIterator<T> for AlkanesVec<T, A>
where
    A: Default,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut vec = Self::new(A::default());
        // Note: We ignore errors here for compatibility with FromIterator trait
        // In a real implementation, you might want a fallible version
        let _ = vec.extend(iter);
        vec
    }
}

// For compatibility with existing code that expects Vec<T>
#[cfg(not(target_arch = "spirv"))]
impl<T, A: AlkanesAllocator> From<AlkanesVec<T, A>> for alloc::vec::Vec<T> {
    fn from(mut alkanes_vec: AlkanesVec<T, A>) -> Self {
        let mut vec = alloc::vec::Vec::with_capacity(alkanes_vec.len());
        for item in alkanes_vec.iter_mut() {
            vec.push(unsafe { ptr::read(item) });
        }
        // Prevent drop of the original elements
        alkanes_vec.len = 0;
        vec
    }
}

#[cfg(not(target_arch = "spirv"))]
impl<T, A: AlkanesAllocator + Default> From<alloc::vec::Vec<T>> for AlkanesVec<T, A> {
    fn from(vec: alloc::vec::Vec<T>) -> Self {
        let mut alkanes_vec = Self::with_capacity(vec.len(), A::default())
            .unwrap_or_else(|_| Self::new(A::default()));
        let _ = alkanes_vec.extend(vec);
        alkanes_vec
    }
}