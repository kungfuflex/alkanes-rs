//! Small vector implementation that stores small arrays inline
//! 
//! This module provides a SmallVec-like data structure that can store
//! small arrays inline to avoid heap allocation for small collections.

use alkanes_alloc::AlkanesAllocator;
use core::{
    fmt,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Deref, DerefMut, Index, IndexMut},
    ptr,
    slice,
};

/// A vector that stores small arrays inline to avoid heap allocation
pub struct AlkanesSmallVec<T, A: AlkanesAllocator, const N: usize> {
    /// Inline storage for small arrays
    inline: [MaybeUninit<T>; N],
    /// Heap storage for larger arrays
    heap: Option<(*mut T, usize)>, // (ptr, capacity)
    /// Current length
    len: usize,
    /// Allocator for heap storage
    allocator: A,
    /// Phantom data for T
    _marker: PhantomData<T>,
}

impl<T, A: AlkanesAllocator, const N: usize> AlkanesSmallVec<T, A, N> {
    /// Creates a new empty small vector
    pub fn new(allocator: A) -> Self {
        Self {
            inline: unsafe { MaybeUninit::uninit().assume_init() },
            heap: None,
            len: 0,
            allocator,
            _marker: PhantomData,
        }
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
        if let Some((_, cap)) = self.heap {
            cap
        } else {
            N
        }
    }

    /// Returns true if the vector is using inline storage
    #[inline]
    fn is_inline(&self) -> bool {
        self.heap.is_none()
    }

    /// Pushes an element to the end of the vector
    pub fn push(&mut self, value: T) -> Result<(), A::Error> {
        if self.len < N && self.is_inline() {
            // Store in inline array
            unsafe {
                self.inline[self.len].write(value);
            }
            self.len += 1;
            Ok(())
        } else {
            // Need to use heap storage
            if self.is_inline() {
                // First time moving to heap - allocate and copy inline data
                self.move_to_heap()?;
            } else if let Some((ptr, cap)) = self.heap {
                // Already on heap, check if we need to grow
                if self.len >= cap {
                    let new_cap = cap * 2;
                    let new_size = new_cap * mem::size_of::<T>();
                    let old_size = cap * mem::size_of::<T>();
                    let align = mem::align_of::<T>();
                    
                    let new_ptr = unsafe {
                        self.allocator.reallocate(ptr as *mut u8, old_size, new_size, align)? as *mut T
                    };
                    self.heap = Some((new_ptr, new_cap));
                }
            }

            // Push to heap
            if let Some((ptr, _)) = self.heap {
                unsafe {
                    ptr::write(ptr.add(self.len), value);
                }
                self.len += 1;
            }
            Ok(())
        }
    }

    /// Moves inline data to heap storage
    fn move_to_heap(&mut self) -> Result<(), A::Error> {
        let initial_cap = (N * 2).max(4);
        let size = initial_cap * mem::size_of::<T>();
        let align = mem::align_of::<T>();
        
        let ptr = self.allocator.allocate(size, align)? as *mut T;
        
        // Copy inline data to heap
        unsafe {
            for i in 0..self.len {
                let value = self.inline[i].assume_init_read();
                ptr::write(ptr.add(i), value);
            }
        }
        
        self.heap = Some((ptr, initial_cap));
        Ok(())
    }

    /// Pops an element from the end of the vector
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                if self.is_inline() {
                    Some(self.inline[self.len].assume_init_read())
                } else if let Some((ptr, _)) = self.heap {
                    Some(ptr::read(ptr.add(self.len)))
                } else {
                    unreachable!()
                }
            }
        }
    }

    /// Clears all elements from the vector
    pub fn clear(&mut self) {
        unsafe {
            if self.is_inline() {
                for i in 0..self.len {
                    self.inline[i].assume_init_drop();
                }
            } else if let Some((ptr, _)) = self.heap {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, self.len));
            }
        }
        self.len = 0;
    }

    /// Returns a slice containing all elements
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            if self.is_inline() {
                slice::from_raw_parts(self.inline.as_ptr() as *const T, self.len)
            } else if let Some((ptr, _)) = self.heap {
                slice::from_raw_parts(ptr, self.len)
            } else {
                slice::from_raw_parts(ptr::null(), 0)
            }
        }
    }

    /// Returns a mutable slice containing all elements
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            if self.is_inline() {
                slice::from_raw_parts_mut(self.inline.as_mut_ptr() as *mut T, self.len)
            } else if let Some((ptr, _)) = self.heap {
                slice::from_raw_parts_mut(ptr, self.len)
            } else {
                slice::from_raw_parts_mut(ptr::null_mut(), 0)
            }
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
        for item in iter {
            self.push(item)?;
        }
        Ok(())
    }

    /// Creates a default instance (for compatibility)
    pub fn default() -> Self
    where
        A: Default,
    {
        Self::new(A::default())
    }
}

impl<T, A: AlkanesAllocator, const N: usize> Drop for AlkanesSmallVec<T, A, N> {
    fn drop(&mut self) {
        self.clear();
        
        if let Some((ptr, cap)) = self.heap {
            let size = cap * mem::size_of::<T>();
            let align = mem::align_of::<T>();
            unsafe {
                self.allocator.deallocate(ptr as *mut u8, size, align);
            }
        }
    }
}

impl<T, A: AlkanesAllocator, const N: usize> Deref for AlkanesSmallVec<T, A, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, A: AlkanesAllocator, const N: usize> DerefMut for AlkanesSmallVec<T, A, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, A: AlkanesAllocator, const N: usize> Index<usize> for AlkanesSmallVec<T, A, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T, A: AlkanesAllocator, const N: usize> IndexMut<usize> for AlkanesSmallVec<T, A, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<T, A: AlkanesAllocator, const N: usize> fmt::Debug for AlkanesSmallVec<T, A, N>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T, A: AlkanesAllocator, const N: usize> PartialEq for AlkanesSmallVec<T, A, N>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        // Manual element-by-element comparison to avoid raw_eq intrinsic for SPIR-V compatibility
        let self_slice = self.as_slice();
        let other_slice = other.as_slice();
        if self_slice.len() != other_slice.len() {
            return false;
        }
        for i in 0..self_slice.len() {
            if self_slice[i] != other_slice[i] {
                return false;
            }
        }
        true
    }
}

impl<T, A: AlkanesAllocator, const N: usize> Eq for AlkanesSmallVec<T, A, N> where T: Eq {}

// Type alias for convenience with default allocator
use alkanes_alloc::DefaultAllocator;
pub type SmallVec<T, const N: usize> = AlkanesSmallVec<T, DefaultAllocator, N>;