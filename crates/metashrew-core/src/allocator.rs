//! Custom preallocated allocator for deterministic memory layout in WASM
//!
//! This module provides a custom bump allocator that uses a preallocated memory region
//! to ensure deterministic memory layout for WASM execution. This is critical for
//! consistent behavior across different WASM runtimes and environments.

#[cfg(feature = "allocator")]
use std::alloc::{GlobalAlloc, Layout};
#[cfg(feature = "allocator")]
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
#[cfg(feature = "allocator")]
use std::sync::RwLock;

/// Detect available memory and determine appropriate cache size
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn detect_available_memory() -> usize {
    // Try to detect available memory by attempting progressively smaller allocations
    // Start with much more conservative sizes for WASM environments
    let test_sizes = [
        1024 * 1024 * 1024, // 1GB (target)
        512 * 1024 * 1024,  // 512MB
        256 * 1024 * 1024,  // 256MB
        128 * 1024 * 1024,  // 128MB
        64 * 1024 * 1024,   // 64MB
        32 * 1024 * 1024,   // 32MB
        16 * 1024 * 1024,   // 16MB
        8 * 1024 * 1024,    // 8MB (absolute minimum)
    ];

    for &size in &test_sizes {
        // Try to allocate a test vector to see if this size is feasible
        // Use try_reserve_exact to avoid capacity overflow panics
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut test_vec = Vec::new();
            // Use try_reserve_exact to safely test allocation
            match test_vec.try_reserve_exact(size) {
                Ok(()) => {
                    // Try to actually allocate a small portion to test if the capacity is realistic
                    let test_allocation_size = (size / 1000).max(1024).min(1024 * 1024); // Test with 0.1% or at least 1KB, max 1MB
                    test_vec.resize(test_allocation_size, 0);
                    test_vec.shrink_to_fit(); // Release the test allocation
                    true
                }
                Err(_) => false,
            }
        })) {
            Ok(true) => {
                println!(
                    "INFO: Detected feasible LRU cache size: {} bytes ({} MB)",
                    size,
                    size / (1024 * 1024)
                );
                return size;
            }
            Ok(false) | Err(_) => {
                println!(
                    "DEBUG: Failed to allocate {} bytes for LRU cache, trying smaller size",
                    size
                );
                continue;
            }
        }
    }

    // If all sizes fail, use a very conservative fallback but warn about it
    let fallback_size = 4 * 1024 * 1024; // 4MB absolute minimum for WASM
    println!("WARNING: Could not allocate any of the preferred cache sizes, falling back to {} bytes ({} MB). Performance may be degraded.",
               fallback_size, fallback_size / (1024 * 1024));
    fallback_size
}

/// Actual memory limit determined at runtime based on available memory
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
static ACTUAL_LRU_CACHE_MEMORY_LIMIT: std::sync::LazyLock<usize> =
    std::sync::LazyLock::new(|| detect_available_memory());

/// Preallocated memory region for LRU cache to ensure consistent memory layout
/// This is allocated at startup to guarantee the same memory addresses regardless
/// of whether the cache is actually used or not.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
static PREALLOCATED_CACHE_MEMORY: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    let actual_limit = *ACTUAL_LRU_CACHE_MEMORY_LIMIT;

    // Try progressively smaller allocations to find what works in WASM environment
    let allocation_attempts = [
        actual_limit,     // Try the detected limit first
        actual_limit / 2, // Try half
        actual_limit / 4, // Try quarter
        16 * 1024 * 1024, // 16MB fallback
        8 * 1024 * 1024,  // 8MB fallback
        4 * 1024 * 1024,  // 4MB fallback
        1024 * 1024,      // 1MB minimal
    ];

    for &size in &allocation_attempts {
        // Try to allocate using try_reserve_exact to avoid capacity overflow panics
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut memory = Vec::new();
            match memory.try_reserve_exact(size) {
                Ok(()) => {
                    // Actually allocate the memory by filling it with zeros
                    // This forces the OS to commit the memory pages immediately
                    memory.resize(size, 0);
                    Some(memory)
                }
                Err(_) => None,
            }
        })) {
            Ok(Some(memory)) => {
                if size < actual_limit {
                    println!(
                            "WARNING: Preallocated {} bytes ({} MB) LRU cache memory (less than target {} MB) at address: {:p}",
                            memory.len(),
                            memory.len() / (1024 * 1024),
                            actual_limit / (1024 * 1024),
                            memory.as_ptr()
                        );
                } else {
                    println!(
                            "INFO: Successfully preallocated {} bytes ({} MB) LRU cache memory region at address: {:p}",
                            memory.len(),
                            memory.len() / (1024 * 1024),
                            memory.as_ptr()
                        );
                }
                return memory;
            }
            Ok(None) | Err(_) => {
                println!(
                    "DEBUG: Failed to preallocate {} bytes, trying smaller size",
                    size
                );
                continue;
            }
        }
    }

    // If all allocations fail, create an empty vector to avoid panics
    println!("ERROR: Failed to preallocate any cache memory, using empty allocation");
    Vec::new()
});

/// Custom bump allocator that uses the preallocated memory region
///
/// This allocator provides deterministic memory layout by allocating from
/// a preallocated memory region using a simple bump allocation strategy.
/// It's designed specifically for LRU cache usage in WASM environments.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub struct PreallocatedBumpAllocator {
    /// Current offset within the preallocated memory region
    offset: AtomicUsize,
    /// Base pointer to the preallocated memory (thread-safe)
    base_ptr: AtomicPtr<u8>,
    /// Total size of the preallocated memory region
    total_size: AtomicUsize,
}

// SAFETY: PreallocatedBumpAllocator is safe to send between threads
// because it uses atomic operations for all mutable state
#[cfg(feature = "allocator")]
unsafe impl Send for PreallocatedBumpAllocator {}

// SAFETY: PreallocatedBumpAllocator is safe to share between threads
// because all operations are atomic and the base pointer is immutable after initialization
#[cfg(feature = "allocator")]
unsafe impl Sync for PreallocatedBumpAllocator {}

#[cfg(feature = "allocator")]
impl PreallocatedBumpAllocator {
    /// Create a new bump allocator using the preallocated memory region
    pub fn new() -> Self {
        // Force initialization of preallocated memory
        let base_ptr = PREALLOCATED_CACHE_MEMORY.as_ptr() as *mut u8;
        let total_size = PREALLOCATED_CACHE_MEMORY.len();
        
        println!("DEBUG: PreallocatedBumpAllocator initialized with base_ptr={:p}, size={} bytes",
                 base_ptr, total_size);
        
        Self {
            offset: AtomicUsize::new(0),
            base_ptr: AtomicPtr::new(base_ptr),
            total_size: AtomicUsize::new(total_size),
        }
    }
    
    /// Get current memory usage statistics
    pub fn get_usage_stats(&self) -> (usize, usize, f64) {
        let used = self.offset.load(Ordering::Relaxed);
        let total = self.total_size.load(Ordering::Relaxed);
        let percentage = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        (used, total, percentage)
    }
    
    /// Reset the allocator (for testing purposes)
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Relaxed);
        println!("DEBUG: PreallocatedBumpAllocator reset");
    }
}

#[cfg(feature = "allocator")]
unsafe impl GlobalAlloc for PreallocatedBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        
        let total_size = self.total_size.load(Ordering::Relaxed);
        let base_ptr = self.base_ptr.load(Ordering::Relaxed);
        
        // If no preallocated memory available, fall back to system allocator
        if total_size == 0 || base_ptr.is_null() {
            return std::alloc::System.alloc(layout);
        }
        
        // Align the current offset to the required alignment
        let current_offset = self.offset.load(Ordering::Relaxed);
        let aligned_offset = (current_offset + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;
        
        // Check if we have enough space
        if new_offset > total_size {
            // Out of preallocated memory, fall back to system allocator
            println!("DEBUG: PreallocatedBumpAllocator out of space (need {} bytes, have {} remaining), falling back to system allocator",
                     size, total_size - current_offset);
            return std::alloc::System.alloc(layout);
        }
        
        // Try to atomically update the offset
        match self.offset.compare_exchange_weak(
            current_offset,
            new_offset,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                let ptr = base_ptr.add(aligned_offset);
                println!("DEBUG: PreallocatedBumpAllocator allocated {} bytes at offset {} (ptr={:p})",
                         size, aligned_offset, ptr);
                ptr
            }
            Err(_) => {
                // Another thread updated the offset, retry
                self.alloc(layout)
            }
        }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let base_ptr = self.base_ptr.load(Ordering::Relaxed);
        let total_size = self.total_size.load(Ordering::Relaxed);
        
        if base_ptr.is_null() || total_size == 0 {
            // No preallocated memory, delegate to system allocator
            std::alloc::System.dealloc(ptr, layout);
            return;
        }
        
        // Check if this pointer is within our preallocated region
        let base = base_ptr;
        let end = base.add(total_size);
        
        if ptr >= base && ptr < end {
            // This is from our preallocated region - bump allocators don't support individual deallocation
            // We just track this for debugging but don't actually free anything
            // The entire preallocated region will be freed when the process exits
        } else {
            // This is from system allocator, delegate deallocation
            std::alloc::System.dealloc(ptr, layout);
        }
    }
}

/// Global instance of our custom allocator (lazy initialization)
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
static PREALLOCATED_ALLOCATOR: std::sync::LazyLock<PreallocatedBumpAllocator> =
    std::sync::LazyLock::new(|| PreallocatedBumpAllocator::new());

/// Flag to control whether to use the preallocated allocator
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
static USE_PREALLOCATED_ALLOCATOR: RwLock<bool> = RwLock::new(false);

/// Wrapper allocator that conditionally uses preallocated memory
///
/// This allocator checks the USE_PREALLOCATED_ALLOCATOR flag and either
/// uses our custom bump allocator or falls back to the system allocator.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub struct ConditionalPreallocatedAllocator;

#[cfg(feature = "allocator")]
unsafe impl GlobalAlloc for ConditionalPreallocatedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if *USE_PREALLOCATED_ALLOCATOR.read().unwrap() {
            PREALLOCATED_ALLOCATOR.alloc(layout)
        } else {
            std::alloc::System.alloc(layout)
        }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if *USE_PREALLOCATED_ALLOCATOR.read().unwrap() {
            PREALLOCATED_ALLOCATOR.dealloc(ptr, layout)
        } else {
            std::alloc::System.dealloc(ptr, layout)
        }
    }
}

/// Set our conditional allocator as the global allocator
///
/// This allows us to control whether allocations use preallocated memory
/// or the system allocator based on the USE_PREALLOCATED_ALLOCATOR flag.
/// Only enabled when the "allocator" feature is active.
#[cfg(feature = "allocator")]
#[global_allocator]
static GLOBAL: ConditionalPreallocatedAllocator = ConditionalPreallocatedAllocator;

/// Enable the use of preallocated memory allocator for LRU cache
///
/// This function enables the custom bump allocator that uses the preallocated
/// memory region. This provides deterministic memory layout for WASM environments.
///
/// **IMPORTANT**: This should be called before any LRU cache operations.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn enable_preallocated_allocator() {
    let mut use_allocator = USE_PREALLOCATED_ALLOCATOR.write().unwrap();
    *use_allocator = true;
    
    // Force initialization of the allocator
    let _ = &*PREALLOCATED_ALLOCATOR;
    
    println!("INFO: Preallocated memory allocator enabled for LRU cache");
}

/// Disable the use of preallocated memory allocator
///
/// This function disables the custom allocator and falls back to system allocation.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn disable_preallocated_allocator() {
    let mut use_allocator = USE_PREALLOCATED_ALLOCATOR.write().unwrap();
    *use_allocator = false;
    
    println!("INFO: Preallocated memory allocator disabled, using system allocator");
}

/// Check if preallocated allocator is enabled
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn is_preallocated_allocator_enabled() -> bool {
    *USE_PREALLOCATED_ALLOCATOR.read().unwrap()
}

/// Get allocator usage statistics
///
/// Returns (used_bytes, total_bytes, usage_percentage)
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn get_allocator_usage_stats() -> (usize, usize, f64) {
    if is_preallocated_allocator_enabled() {
        PREALLOCATED_ALLOCATOR.get_usage_stats()
    } else {
        (0, 0, 0.0)
    }
}

/// Reset the preallocated allocator (for testing)
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn reset_preallocated_allocator() {
    if is_preallocated_allocator_enabled() {
        PREALLOCATED_ALLOCATOR.reset();
    }
}

/// Clean shutdown of the preallocated allocator
///
/// This function should be called at the end of tests to ensure proper cleanup.
/// It disables the allocator and clears any cached state.
///
/// NOTE: We don't reset the bump allocator because that would invalidate
/// existing pointers that may still be in use. Instead, we just disable
/// it for new allocations and clear the caches.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn shutdown_preallocated_allocator() {
    // Clear all LRU caches to release any references to preallocated memory
    // This must be done BEFORE disabling the allocator to ensure proper cleanup
    crate::clear();
    
    // Disable the preallocated allocator for new allocations
    // This will cause new allocations to use the system allocator
    disable_preallocated_allocator();
    
    // NOTE: We intentionally do NOT reset the bump allocator here because:
    // 1. Bump allocators cannot safely deallocate individual allocations
    // 2. Resetting would invalidate existing pointers that may still be in use
    // 3. The memory will be reclaimed when the process exits
    
    println!("DEBUG: Preallocated allocator shutdown completed");
}

/// Get the actual LRU cache memory limit determined at runtime
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn get_actual_lru_cache_memory_limit() -> usize {
    *ACTUAL_LRU_CACHE_MEMORY_LIMIT
}

/// Ensure the preallocated memory is initialized
/// This function forces the lazy static to initialize, ensuring the memory
/// is allocated before any other operations that might affect memory layout.
/// Only available when the "allocator" feature is enabled.
#[cfg(feature = "allocator")]
pub fn ensure_preallocated_memory() {
    // Access the lazy static to force initialization
    let memory_ptr = PREALLOCATED_CACHE_MEMORY.as_ptr();
    let memory_size = PREALLOCATED_CACHE_MEMORY.len();
    let actual_limit = *ACTUAL_LRU_CACHE_MEMORY_LIMIT;

    println!(
        "DEBUG: LRU cache preallocated memory confirmed (indexer mode): ptr={:p}, size={} bytes, target_limit={} bytes",
        memory_ptr,
        memory_size,
        actual_limit
    );

    // Verify the memory is actually allocated by touching the first and last pages
    if memory_size > 0 {
        unsafe {
            // Touch first page
            std::ptr::read_volatile(memory_ptr);
            // Touch last page if we have more than one byte
            if memory_size > 1 {
                std::ptr::read_volatile(memory_ptr.add(memory_size - 1));
            }
        }

        println!("INFO: LRU cache memory preallocation verified and committed (indexer mode): {} bytes ({} MB)",
                  memory_size, memory_size / (1024 * 1024));
    } else {
        println!("WARNING: LRU cache memory preallocation failed - using dynamic allocation only");
    }
}

// Stub functions when allocator feature is not enabled
#[cfg(not(feature = "allocator"))]
pub fn enable_preallocated_allocator() {
    // No-op when allocator feature is disabled
}

#[cfg(not(feature = "allocator"))]
pub fn disable_preallocated_allocator() {
    // No-op when allocator feature is disabled
}

#[cfg(not(feature = "allocator"))]
pub fn is_preallocated_allocator_enabled() -> bool {
    false
}

#[cfg(not(feature = "allocator"))]
pub fn get_allocator_usage_stats() -> (usize, usize, f64) {
    (0, 0, 0.0)
}

#[cfg(not(feature = "allocator"))]
pub fn reset_preallocated_allocator() {
    // No-op when allocator feature is disabled
}

#[cfg(not(feature = "allocator"))]
pub fn shutdown_preallocated_allocator() {
    // Clear all LRU caches but don't do allocator-specific cleanup
    crate::clear();
}

#[cfg(not(feature = "allocator"))]
pub fn get_actual_lru_cache_memory_limit() -> usize {
    // Return a reasonable default when allocator feature is disabled
    // Don't do any actual memory detection or preallocation
    1024 * 1024 * 1024 // 1GB default
}

/// Stub version of detect_available_memory for when allocator feature is disabled
#[cfg(not(feature = "allocator"))]
pub fn detect_available_memory() -> usize {
    // Don't do any actual memory detection when allocator feature is disabled
    1024 * 1024 * 1024 // 1GB default
}

#[cfg(not(feature = "allocator"))]
pub fn ensure_preallocated_memory() {
    // No-op when allocator feature is disabled
    println!("INFO: Preallocated memory not available (allocator feature not enabled)");
}
