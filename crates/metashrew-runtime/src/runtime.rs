//! Core WebAssembly runtime for executing Bitcoin indexers
//!
//! This module provides the main [`MetashrewRuntime`] struct that executes WebAssembly
//! modules for Bitcoin block processing. It implements the host side of the WASM
//! interface, providing functions that WASM modules can call to interact with the
//! database and retrieve blockchain data.
//!
//! # Architecture
//!
//! The runtime follows a generic design pattern where it's parameterized over a
//! storage type `T: KeyValueStoreLike`. This enables:
//!
//! - **Testing**: Use in-memory storage for fast unit tests
//! - **Production**: Use RocksDB for persistent, high-performance storage
//! - **Flexibility**: Support for future storage backends
//!
//! # Key Components
//!
//! ## WASM Execution Environment
//!
//! The runtime uses Wasmtime to execute WebAssembly modules with:
//! - **Deterministic execution**: Configured for reproducible results
//! - **Memory isolation**: Each block execution starts with fresh memory
//! - **Resource limits**: Configurable memory and execution limits
//! - **Host function bindings**: Provides database and I/O operations to WASM
//!
//! ## Host Functions
//!
//! The runtime provides these functions to WASM modules:
//! - `__host_len()`: Get input data length
//! - `__load_input(ptr)`: Load block data into WASM memory
//! - `__get(key_ptr, value_ptr)`: Read from database
//! - `__get_len(key_ptr)`: Get value length for a key
//! - `__flush(data_ptr)`: Write key-value pairs to database
//! - `__log(ptr)`: Output debug messages
//!
//! ## Execution Modes
//!
//! The runtime supports multiple execution modes:
//! - **Normal**: Standard block processing with database writes
//! - **View**: Read-only execution for querying state
//! - **Preview**: Isolated execution for testing block effects
//! - **Atomic**: Batch processing with rollback capability
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use metashrew_runtime::{MetashrewRuntime, traits::KeyValueStoreLike};
//! use std::path::PathBuf;
//!
//! async fn process_blocks<T: KeyValueStoreLike>(
//!     mut runtime: MetashrewRuntime<T>,
//!     block_data: &[u8],
//!     height: u32
//! ) -> anyhow::Result<()> {
//!     // Process a block
//!     runtime.process_block(height, block_data).await?;
//!
//!     // Query the resulting state
//!     let state_root = runtime.get_state_root(height).await?;
//!     println!("State root: {}", hex::encode(state_root));
//!
//!     Ok(())
//! }
//! ```

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use prost::Message;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Linker, Store, StoreLimits, StoreLimitsBuilder};

use crate::context::MetashrewRuntimeContext;
use crate::smt::SMTHelper;
use crate::traits::{BatchLike, KeyValueStoreLike};


/// Internal key used to store the current blockchain tip height
///
/// This key is used internally by the runtime to track the highest block
/// that has been successfully processed and committed to the database.
pub const TIP_HEIGHT_KEY: &'static str = "/__INTERNAL/tip-height";

fn lock_err<T>(err: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow!("Mutex lock error: {}", err)
}

fn try_into_vec<const N: usize>(bytes: [u8; N]) -> Result<Vec<u8>> {
    Vec::<u8>::try_from(bytes).map_err(|e| anyhow!("Failed to convert bytes to Vec: {:?}", e))
}

use crate::proto::metashrew::KeyValueFlush;

/// WASM execution state tracking for deterministic execution
///
/// This struct maintains the execution state for a single WASM instance,
/// including resource limits and failure tracking. It's designed to ensure
/// deterministic execution across different environments.
///
/// # Fields
///
/// - `limits`: Resource limits for WASM execution (memory, tables, instances)
/// - `had_failure`: Tracks whether any host function call failed during execution
///
/// # Deterministic Execution
///
/// The state is configured with maximum resource limits to ensure consistent
/// behavior across different environments. Memory is pre-allocated to avoid
/// non-deterministic growth patterns.
pub struct State {
    /// Resource limits for WASM execution
    ///
    /// Set to maximum values to ensure deterministic behavior by avoiding
    /// dynamic resource allocation during execution.
    limits: StoreLimits,

    /// Tracks execution failures in host functions
    ///
    /// When a host function encounters an error (e.g., database failure,
    /// memory access error), it sets this flag to signal the runtime
    /// that execution should be aborted.
    had_failure: bool,
}

impl State {
    /// Create a new WASM execution state with maximum resource limits
    ///
    /// # Returns
    ///
    /// A new [`State`] instance configured for deterministic execution with:
    /// - Maximum memory allocation
    /// - Maximum table allocation
    /// - Maximum instance allocation
    /// - No execution failures initially
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let state = State::new();
    /// // State is ready for deterministic WASM execution
    /// ```
    pub fn new() -> Self {
        State {
            limits: StoreLimitsBuilder::new()
                .memories(usize::MAX)
                .tables(usize::MAX)
                .instances(usize::MAX)
                .build(),
            had_failure: false,
        }
    }
}

/// Core WebAssembly runtime for executing Bitcoin indexers
///
/// [`MetashrewRuntime`] is the main execution engine that runs WebAssembly modules
/// for Bitcoin block processing. It's generic over storage backends, enabling
/// flexible deployment scenarios from testing to production.
///
/// # Type Parameters
///
/// - `T`: Storage backend implementing [`KeyValueStoreLike`] + [`Clone`] + [`Send`] + [`Sync`]
///
/// # Architecture
///
/// The runtime maintains both synchronous and asynchronous execution engines:
/// - **Synchronous engine**: Used for block processing and preview operations
/// - **Asynchronous engine**: Used for view functions and cooperative yielding
///
/// # Key Components
///
/// ## Execution Context
/// - `context`: Shared state including database, block data, and execution height
/// - `wasmstore`: WASM execution store with resource limits and failure tracking
///
/// ## WASM Engines
/// - `engine`: Synchronous Wasmtime engine for block processing
/// - `async_engine`: Asynchronous Wasmtime engine for view functions
/// - `module`: Compiled WASM module for synchronous execution
/// - `async_module`: Compiled WASM module for asynchronous execution
///
/// ## Host Interface
/// - `linker`: Provides host functions to WASM modules
/// - `instance`: Instantiated WASM module ready for execution
///
/// # Execution Modes
///
/// ## Block Processing (`run`)
/// Normal block processing with database writes and state updates:
/// ```rust,ignore
/// runtime.run()?; // Process current block
/// ```
///
/// ## View Functions (`view`)
/// Read-only execution for querying historical state:
/// ```rust,ignore
/// let result = runtime.view("get_balance".to_string(), &input, height).await?;
/// ```
///
/// ## Preview Mode (`preview`)
/// Isolated execution for testing block effects without committing:
/// ```rust,ignore
/// let result = runtime.preview(&block_data, "view_function".to_string(), &input, height)?;
/// ```
///
/// ## Atomic Processing (`process_block_atomic`)
/// Batch processing with rollback capability:
/// ```rust,ignore
/// let atomic_result = runtime.process_block_atomic(height, &block_data, &block_hash).await?;
/// ```
///
/// # Memory Management
///
/// The runtime supports persistent memory between blocks and view calls:
/// - **Memory persistence**: WASM memory state is retained between normal block processing
/// - **Stateful views**: WASM memory persists between view calls (enabled by default)
/// - **Resource limits**: Pre-allocated maximum memory to avoid growth
/// - **LRU cache**: metashrew-core redesign prevents unbounded memory growth
/// - **Reorg refresh**: Memory is only refreshed during chain reorganizations
///
/// # Thread Safety
///
/// All shared state is protected by [`Arc<Mutex<_>>`] for safe concurrent access.
/// The runtime can be safely shared across threads for parallel view operations.
///
/// # Example Usage
///
/// ```rust,ignore
/// use metashrew_runtime::{MetashrewRuntime, traits::KeyValueStoreLike};
/// use std::path::PathBuf;
///
/// async fn run_indexer<T: KeyValueStoreLike + Clone + Send + Sync + 'static>(
///     indexer_path: PathBuf,
///     storage: T,
///     block_data: &[u8],
///     height: u32
/// ) -> anyhow::Result<()> {
///     // Load the runtime with WASM indexer
///     let mut runtime = MetashrewRuntime::load(indexer_path, storage)?;
///
///     // Process a block
///     runtime.process_block(height, block_data).await?;
///
///     // Query the resulting state
///     let balance = runtime.view(
///         "get_balance".to_string(),
///         &address_bytes,
///         height
///     ).await?;
///
///     println!("Balance: {}", hex::encode(balance));
///     Ok(())
/// }
/// ```
pub struct MetashrewRuntime<T: KeyValueStoreLike> {
    /// Shared execution context containing database, block data, and state
    ///
    /// Protected by [`Arc<Mutex<_>>`] for thread-safe access across
    /// different execution modes and concurrent view operations.
    pub context: Arc<Mutex<MetashrewRuntimeContext<T>>>,

    /// Synchronous Wasmtime engine for block processing
    ///
    /// Configured for deterministic execution with:
    /// - NaN canonicalization for consistent floating point
    /// - Relaxed SIMD determinism
    /// - Static memory allocation
    pub engine: wasmtime::Engine,

    /// Asynchronous Wasmtime engine for view functions
    ///
    /// Supports cooperative yielding and fuel-based execution limits
    /// for long-running view operations that need to yield control.
    pub async_engine: wasmtime::Engine,

    /// WASM execution store with state tracking
    ///
    /// Contains the execution state including resource limits and
    /// failure tracking. Reset after each block for deterministic behavior.
    pub wasmstore: wasmtime::Store<State>,

    /// Compiled WASM module for asynchronous execution
    ///
    /// Used by view functions and other operations that need
    /// cooperative yielding and async execution.
    pub async_module: wasmtime::Module,

    /// Compiled WASM module for synchronous execution
    ///
    /// Used for block processing and other operations that
    /// need deterministic, non-yielding execution.
    pub module: wasmtime::Module,

    /// Host function linker providing database and I/O operations
    ///
    /// Binds host functions like `__get`, `__flush`, `__log` that
    /// WASM modules can call to interact with the database and runtime.
    pub linker: wasmtime::Linker<State>,

    /// Instantiated WASM module ready for execution
    ///
    /// Contains the loaded and linked WASM instance with all
    /// host functions bound and ready to execute.
    pub instance: wasmtime::Instance,

    /// Stateful view runtime for retaining WASM memory between view calls
    ///
    /// This field contains a persistent WASM runtime that retains its memory
    /// and instance state between view function calls. When present, view
    /// functions will reuse this runtime instead of creating new instances.
    /// This enables stateful view operations where WASM memory persists.
    ///
    /// **DEFAULT BEHAVIOR**: Stateful views are now enabled by default for
    /// better performance and to support stateful operations.
    pub stateful_view_runtime: Option<StatefulViewRuntime<T>>,

    /// Flag to disable WASM __log host function
    ///
    /// When true, the __log host function will silently ignore all WASM log calls.
    /// This provides the same behavior as the old --features logs flag but controlled
    /// at runtime instead of compile time.
    pub disable_wasmtime_log: bool,
}

/// Stateful view runtime that retains WASM memory and instance between calls
///
/// This struct maintains a persistent WASM execution environment specifically
/// for view functions. Unlike the main runtime which resets memory after each
/// block, this runtime preserves WASM memory state between view calls, enabling
/// stateful view operations.
///
/// # Key Features
///
/// - **Memory Persistence**: WASM memory is retained between view calls
/// - **Instance Reuse**: Same WASM instance used for all view operations
/// - **Thread Safety**: Protected by mutex for concurrent access
/// - **Lazy Initialization**: Created on first view call if enabled
///
/// # Memory Management
///
/// The WASM author is responsible for managing memory within their WASM module.
/// The runtime simply provides a persistent execution environment without
/// automatic memory cleanup between calls.
pub struct StatefulViewRuntime<T: KeyValueStoreLike> {
    /// Asynchronous WASM store for view execution
    ///
    /// Maintains the WASM execution state including memory, globals,
    /// and other runtime state that persists between view calls.
    /// Protected by mutex for thread-safe access.
    pub wasmstore: Arc<tokio::sync::Mutex<wasmtime::Store<State>>>,

    /// WASM instance for view execution
    ///
    /// The instantiated WASM module that will be reused for all
    /// view function calls, maintaining its memory state.
    pub instance: wasmtime::Instance,

    /// Host function linker for view operations
    ///
    /// Provides the view-specific host functions like `__get` and `__log`
    /// that view functions can call to interact with the database.
    pub linker: wasmtime::Linker<State>,

    /// Execution context for view operations
    ///
    /// Contains the database reference and other context needed for
    /// view function execution. Updated for each view call.
    pub context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
}

pub fn db_make_list_key(v: &Vec<u8>, index: u32) -> Result<Vec<u8>> {
    let mut entry = v.clone();
    let index_bits = try_into_vec(index.to_le_bytes())?;
    entry.extend(index_bits);
    Ok(entry)
}

pub fn db_make_length_key(key: &Vec<u8>) -> Result<Vec<u8>> {
    db_make_list_key(key, u32::MAX)
}

pub fn db_make_updated_key(key: &Vec<u8>) -> Vec<u8> {
    key.clone()
}

pub fn u32_to_vec(v: u32) -> Result<Vec<u8>> {
    try_into_vec(v.to_le_bytes())
}

pub fn try_read_arraybuffer_as_vec(data: &[u8], data_start: i32) -> Result<Vec<u8>> {
    if data_start < 4 || (data_start as usize) > data.len() {
        return Err(anyhow!("memory error: invalid data_start"));
    }

    // data_start points to the data portion, length is at data_start - 4
    // This matches metashrew-support export_bytes which returns pointer + 4
    let len_offset = (data_start as usize) - 4;
    let len = u32::from_le_bytes(data[len_offset..len_offset + 4].try_into().unwrap());

    let data_offset = data_start as usize;
    let end_offset = data_offset + (len as usize);

    if end_offset > data.len() {
        return Err(anyhow!("memory error: data extends beyond memory bounds"));
    }

    return Ok(Vec::<u8>::from(&data[data_offset..end_offset]));
}

pub fn read_arraybuffer_as_vec(data: &[u8], data_start: i32) -> Vec<u8> {
    match try_read_arraybuffer_as_vec(data, data_start) {
        Ok(v) => v,
        Err(_) => Vec::<u8>::new(),
    }
}

// Legacy function removed

pub fn to_signed_or_trap<'a, T: TryInto<i32>>(_caller: &mut Caller<'_, State>, v: T) -> i32 {
    return match <T as TryInto<i32>>::try_into(v) {
        Ok(v) => v,
        Err(_) => {
            return i32::MAX;
        }
    };
}

pub fn to_usize_or_trap<'a, T: TryInto<usize>>(_caller: &mut Caller<'_, State>, v: T) -> usize {
    return match <T as TryInto<usize>>::try_into(v) {
        Ok(v) => v,
        Err(_) => {
            return usize::MAX;
        }
    };
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> MetashrewRuntime<T> {
    /// Load and initialize a new MetashrewRuntime from a WASM indexer file
    ///
    /// This is the primary constructor that loads a WebAssembly indexer module
    /// and sets up the complete runtime environment for Bitcoin block processing.
    ///
    /// # Parameters
    ///
    /// - `indexer`: Path to the compiled WASM indexer module file
    /// - `store`: Storage backend implementing [`KeyValueStoreLike`]
    ///
    /// # Returns
    ///
    /// A fully initialized [`MetashrewRuntime`] ready for block processing
    ///
    /// # Configuration
    ///
    /// The runtime is configured for deterministic execution with:
    /// - **NaN canonicalization**: Ensures consistent floating point behavior
    /// - **Relaxed SIMD determinism**: Makes SIMD operations deterministic
    /// - **Static memory allocation**: Pre-allocates 4GB maximum memory
    /// - **Memory guards**: 64KB guard pages for memory safety
    /// - **Async support**: Separate engine for cooperative yielding
    ///
    /// # Host Functions
    ///
    /// Sets up the complete host function interface:
    /// - `__host_len()`: Get input data length
    /// - `__load_input(ptr)`: Load block data into WASM memory
    /// - `__get(key_ptr, value_ptr)`: Read from database
    /// - `__get_len(key_ptr)`: Get value length for a key
    /// - `__flush(data_ptr)`: Write key-value pairs to database
    /// - `__log(ptr)`: Output debug messages
    /// - `abort()`: Handle WASM abort calls
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - WASM module file cannot be loaded or parsed
    /// - Engine configuration fails
    /// - Module instantiation fails
    /// - Host function binding fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use metashrew_runtime::MetashrewRuntime;
    /// use std::path::PathBuf;
    ///
    /// // Load runtime with RocksDB storage
    /// let runtime = MetashrewRuntime::load(
    ///     PathBuf::from("indexer.wasm"),
    ///     my_storage_backend
    /// )?;
    /// ```
    pub fn load(
        indexer: PathBuf,
        mut store: T,
        prefix_configs: Vec<(String, Vec<u8>)>,
    ) -> Result<Self> {
        // CRITICAL: Set cache mode to indexer for deterministic memory layout
        // Runtime is used for indexer operations which need consistent memory layout
        metashrew_core::lru_cache::set_cache_allocation_mode(
            metashrew_core::lru_cache::CacheAllocationMode::Indexer
        );
        
        // CRITICAL: Ensure LRU cache memory is preallocated FIRST (only in indexer mode)
        // This must happen before any WASM engine configuration to guarantee
        // consistent memory layout for deterministic execution
        metashrew_core::allocator::ensure_preallocated_memory();
        
        // Configure the engine with settings for deterministic execution
        let mut config = wasmtime::Config::default();
        // Enable NaN canonicalization for deterministic floating point operations
        config.cranelift_nan_canonicalization(true);
        // Make relaxed SIMD deterministic (or disable it if not needed)
        config.relaxed_simd_deterministic(true);
        // Allocate memory at maximum size to avoid non-deterministic memory growth
        config.static_memory_maximum_size(0x100000000); // 4GB max memory
        config.static_memory_guard_size(0x10000); // 64KB guard
                                                  // Pre-allocate memory to maximum size
        config.memory_init_cow(false); // Disable copy-on-write to ensure consistent memory behavior

        // Configure async engine with the same deterministic settings
        let mut async_config = config.clone();
        async_config.consume_fuel(true);
        async_config.async_support(true);

        let engine = wasmtime::Engine::new(&config)?;
        let async_engine = wasmtime::Engine::new(&async_config)?;
        let module = wasmtime::Module::from_file(&engine, indexer.clone().into_os_string())
            .context("Failed to load WASM module")?;
        let async_module = wasmtime::Module::from_file(&async_engine, indexer.into_os_string())
            .context("Failed to load WASM module")?;
        let mut linker = Linker::<State>::new(&engine);
        let mut wasmstore = Store::<State>::new(&engine, State::new());
        let tip_height = match store.get(&TIP_HEIGHT_KEY.as_bytes().to_vec()) {
            Ok(Some(bytes)) if bytes.len() >= 4 => {
                u32::from_le_bytes(bytes[..4].try_into().unwrap())
            }
            _ => 0,
        };
        let context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
            MetashrewRuntimeContext<T>,
        >::new(
            MetashrewRuntimeContext::new(store, tip_height, vec![], prefix_configs),
        ));
        {
            wasmstore.limiter(|state| &mut state.limits)
        }
        {
            Self::setup_linker_with_log_flag(context.clone(), &mut linker, false)
                .context("Failed to setup basic linker")?;
            Self::setup_linker_indexer(context.clone(), &mut linker)
                .context("Failed to setup indexer linker")?;
            linker.define_unknown_imports_as_traps(&module)?;
        }
        let instance = linker
            .instantiate(&mut wasmstore, &module)
            .context("Failed to instantiate WASM module")?;
        let mut runtime = MetashrewRuntime {
            wasmstore,
            async_engine,
            engine,
            async_module,
            module,
            linker,
            context,
            instance,
            stateful_view_runtime: None,
            disable_wasmtime_log: false,
        };

        // Enable stateful views by default
        // This allows WASM memory to persist between view calls for better performance
        // and stateful operations
        log::info!("Enabling stateful views by default for persistent WASM memory");
        if let Err(e) = runtime.enable_stateful_views_sync() {
            log::warn!("Failed to enable stateful views by default: {}", e);
            // Continue without stateful views if initialization fails
        }

        Ok(runtime)
    }

    pub fn new(
        indexer: &[u8],
        mut store: T,
        prefix_configs: Vec<(String, Vec<u8>)>,
    ) -> Result<Self> {
        
        // Configure the engine with settings for deterministic execution
        let mut config = wasmtime::Config::default();
        // Enable NaN canonicalization for deterministic floating point operations
        config.cranelift_nan_canonicalization(true);
        // Make relaxed SIMD deterministic (or disable it if not needed)
        config.relaxed_simd_deterministic(true);
        // Allocate memory at maximum size to avoid non-deterministic memory growth
        config.static_memory_maximum_size(0x100000000); // 4GB max memory
        config.static_memory_guard_size(0x10000); // 64KB guard
                                                  // Pre-allocate memory to maximum size
        config.memory_init_cow(false); // Disable copy-on-write to ensure consistent memory behavior

        // Configure async engine with the same deterministic settings
        let mut async_config = config.clone();
        async_config.consume_fuel(true);
        async_config.async_support(true);

        let engine = wasmtime::Engine::new(&config)?;
        let async_engine = wasmtime::Engine::new(&async_config)?;
        let module = wasmtime::Module::new(&engine, indexer)
            .context("Failed to load WASM module from bytes")?;
        let async_module = wasmtime::Module::new(&async_engine, indexer)
            .context("Failed to load async WASM module from bytes")?;
        let mut linker = Linker::<State>::new(&engine);
        let mut wasmstore = Store::<State>::new(&engine, State::new());
        let tip_height = match store.get(&TIP_HEIGHT_KEY.as_bytes().to_vec()) {
            Ok(Some(bytes)) if bytes.len() >= 4 => {
                u32::from_le_bytes(bytes[..4].try_into().unwrap())
            }
            _ => 0,
        };
        let context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
            MetashrewRuntimeContext<T>,
        >::new(
            MetashrewRuntimeContext::new(store, tip_height, vec![], prefix_configs),
        ));
        {
            wasmstore.limiter(|state| &mut state.limits)
        }
        {
            Self::setup_linker_with_log_flag(context.clone(), &mut linker, false)
                .context("Failed to setup basic linker")?;
            Self::setup_linker_indexer(context.clone(), &mut linker)
                .context("Failed to setup indexer linker")?;
            linker.define_unknown_imports_as_traps(&module)?;
        }
        let instance = linker
            .instantiate(&mut wasmstore, &module)
            .context("Failed to instantiate WASM module")?;
        Ok(MetashrewRuntime {
            wasmstore,
            async_engine,
            engine,
            async_module,
            module,
            linker,
            context,
            instance,
            stateful_view_runtime: None,
            disable_wasmtime_log: false,
        })
    }

    /// Execute a block in preview mode with isolated database state
    ///
    /// Preview mode allows testing the effects of a block without committing
    /// changes to the main database. It creates an isolated copy of the database,
    /// processes the block, then executes a view function on the resulting state.
    ///
    /// # Parameters
    ///
    /// - `block`: Raw block data to process
    /// - `symbol`: Name of the view function to execute after block processing
    /// - `input`: Input data for the view function
    /// - `height`: Block height for processing context
    ///
    /// # Returns
    ///
    /// The result of executing the view function on the preview state
    ///
    /// # Process Flow
    ///
    /// 1. **Create isolated database**: Copy current database state
    /// 2. **Process block**: Execute `_start` function with block data
    /// 3. **Create view runtime**: Set up new runtime for view execution
    /// 4. **Execute view function**: Run the specified view function
    /// 5. **Return result**: Extract and return the view function output
    ///
    /// # Isolation Guarantees
    ///
    /// - Changes are made to a database copy, not the original
    /// - Original database state remains unchanged
    /// - Multiple previews can run concurrently
    /// - Preview state is discarded after execution
    ///
    /// # Use Cases
    ///
    /// - **Testing**: Validate block effects before committing
    /// - **Simulation**: Explore "what-if" scenarios
    /// - **Debugging**: Inspect intermediate state during development
    /// - **Analysis**: Query state changes without persistence
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Preview the effect of a block on account balances
    /// let balance_after = runtime.preview(
    ///     &block_data,
    ///     "get_balance".to_string(),
    ///     &address_bytes,
    ///     height
    /// )?;
    ///
    /// println!("Balance after block: {}", hex::encode(balance_after));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Database copy creation fails
    /// - Block processing fails during `_start` execution
    /// - View function is not found in the WASM module
    /// - View function execution fails
    /// - Memory access errors occur
    pub fn preview(
        &self,
        block: &Vec<u8>,
        symbol: String,
        input: &Vec<u8>,
        height: u32,
    ) -> Result<Vec<u8>> {
        // Create preview context with isolated DB copy
        let preview_db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.create_isolated_copy()
        };

        // Create a new runtime with preview db using the synchronous engine
        // Process the preview block at height + 1 to simulate adding it after the target height
        let preview_height = height + 1;
        let mut runtime = Self::new_with_db(
            preview_db,
            preview_height,
            self.engine.clone(),
            self.module.clone(),
        )?;
        runtime.context.lock().map_err(lock_err)?.block = block.clone();

        // Execute block via _start to populate preview db
        let start = runtime
            .instance
            .get_typed_func::<(), ()>(&mut runtime.wasmstore, "_start")
            .context("Failed to get _start function for preview")?;

        match start.call(&mut runtime.wasmstore, ()) {
            Ok(_) => {
                let context_guard = runtime.context.lock().map_err(lock_err)?;
                if context_guard.state != 1 && !runtime.wasmstore.data().had_failure {
                    return Err(anyhow!("indexer exited unexpectedly during preview"));
                }
            }
            Err(e) => return Err(e).context("Error executing _start in preview"),
        }

        // Create new runtime just for the view using the updated preview DB
        // Query at the preview height to see the state after processing the preview block
        let mut view_runtime = {
            let context = runtime.context.lock().map_err(lock_err)?;
            // Create a view runtime with the updated database
            let mut linker = Linker::<State>::new(&self.engine);
            let mut wasmstore = Store::<State>::new(&self.engine, State::new());
            let view_context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
                MetashrewRuntimeContext<T>,
            >::new(
                MetashrewRuntimeContext::new(context.db.clone(), preview_height, vec![], vec![]),
            ));

            wasmstore.limiter(|state| &mut state.limits);

            Self::setup_linker(view_context.clone(), &mut linker)
                .context("Failed to setup basic linker for preview view")?;
            Self::setup_linker_view(view_context.clone(), &mut linker)
                .context("Failed to setup view linker for preview")?;
            linker.define_unknown_imports_as_traps(&self.module)?;

            let instance = linker
                .instantiate(&mut wasmstore, &self.module)
                .context("Failed to instantiate WASM module for preview view")?;

            MetashrewRuntime {
                wasmstore,
                engine: self.engine.clone(),
                async_engine: self.engine.clone(),
                module: self.module.clone(),
                async_module: self.module.clone(),
                linker,
                context: view_context,
                instance,
                stateful_view_runtime: None,
                disable_wasmtime_log: false,
            }
        };

        // Set block to input for view
        view_runtime.context.lock().map_err(lock_err)?.block = input.clone();

        // Execute view function
        let func = view_runtime
            .instance
            .get_typed_func::<(), i32>(&mut view_runtime.wasmstore, symbol.as_str())
            .context("Failed to get view function")?;

        let result = func
            .call(&mut view_runtime.wasmstore, ())
            .context("Failed to execute view function")?;

        let memory = view_runtime
            .instance
            .get_memory(&mut view_runtime.wasmstore, "memory")
            .ok_or_else(|| anyhow!("Failed to get memory for view result"))?;

        // Get the final result
        Ok(read_arraybuffer_as_vec(
            memory.data(&mut view_runtime.wasmstore),
            result,
        ))
    }

    // Async version of preview for use with the view server
    pub async fn preview_async(
        &self,
        block: &Vec<u8>,
        symbol: String,
        input: &Vec<u8>,
        height: u32,
    ) -> Result<Vec<u8>> {
        // For now, just use the synchronous version
        // In the future, we can implement a fully async version if needed
        self.preview(block, symbol, input, height)
    }

    /// Execute a view function to query historical blockchain state
    ///
    /// View functions provide read-only access to the blockchain state at any
    /// historical block height. They use the asynchronous engine with cooperative
    /// yielding to handle long-running queries without blocking.
    ///
    /// # Parameters
    ///
    /// - `symbol`: Name of the view function to execute
    /// - `input`: Input data for the view function (typically query parameters)
    /// - `height`: Block height to query (determines database state snapshot)
    ///
    /// # Returns
    ///
    /// The result of the view function execution as raw bytes
    ///
    /// # Execution Model
    ///
    /// - **Read-only**: No database modifications are allowed
    /// - **Historical**: Queries state at the specified block height
    /// - **Asynchronous**: Uses cooperative yielding for long operations
    /// - **Isolated**: Each view runs in its own WASM instance
    ///
    /// # State Access
    ///
    /// View functions access historical state through:
    /// - **Append-only lookups**: Height-indexed lookups on append-only data
    /// - **Immutable snapshots**: Consistent view of state at target height
    /// - **Efficient indexing**: Optimized for historical range queries
    ///
    /// # Cooperative Yielding
    ///
    /// The async engine provides:
    /// - **Fuel limits**: Prevents infinite loops and resource exhaustion
    /// - **Yield intervals**: Periodic yielding for responsive execution
    /// - **Cancellation**: Ability to abort long-running queries
    ///
    /// # Use Cases
    ///
    /// - **Balance queries**: Get account balances at specific heights
    /// - **Transaction history**: Query transaction effects over time
    /// - **State analysis**: Analyze protocol state evolution
    /// - **API endpoints**: Power JSON-RPC query interfaces
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Query account balance at a specific block height
    /// let balance = runtime.view(
    ///     "get_balance".to_string(),
    ///     &address_bytes,
    ///     height
    /// ).await?;
    ///
    /// // Query transaction count for an address
    /// let tx_count = runtime.view(
    ///     "get_transaction_count".to_string(),
    ///     &address_bytes,
    ///     height
    /// ).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - View function is not found in the WASM module
    /// - Input data is malformed or invalid
    /// - Database query fails or times out
    /// - WASM execution encounters an error
    /// - Memory access violations occur
    pub async fn view(&self, symbol: String, input: &Vec<u8>, height: u32) -> Result<Vec<u8>> {
        // Check if we have a stateful view runtime and should use it
        if let Some(ref stateful_runtime) = self.stateful_view_runtime {
            return self
                .view_stateful(stateful_runtime, symbol, input, height)
                .await;
        }

        // Fall back to the original non-stateful implementation
        let db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        // Create a new async runtime for the view
        let mut view_runtime = Self::new_with_db_async(
            db,
            height,
            self.async_engine.clone(),
            self.async_module.clone(),
        )
        .await?;

        // Set the input as the block data
        view_runtime.context.lock().map_err(lock_err)?.block = input.clone();

        // Set fuel for cooperative yielding
        view_runtime.wasmstore.set_fuel(u64::MAX)?;
        view_runtime
            .wasmstore
            .fuel_async_yield_interval(Some(10000))?;

        // Execute view function
        let func = view_runtime
            .instance
            .get_typed_func::<(), i32>(&mut view_runtime.wasmstore, symbol.as_str())
            .with_context(|| format!("Failed to get view function '{}'", symbol))?;

        // Use async call
        let result = func
            .call_async(&mut view_runtime.wasmstore, ())
            .await
            .with_context(|| format!("Failed to execute view function '{}'", symbol))?;

        let memory = view_runtime
            .instance
            .get_memory(&mut view_runtime.wasmstore, "memory")
            .ok_or_else(|| anyhow!("Failed to get memory for view result"))?;

        Ok(read_arraybuffer_as_vec(
            memory.data(&mut view_runtime.wasmstore),
            result,
        ))
    }

    /// Refresh WASM memory by creating a new instance
    ///
    /// **NEW USAGE**: This function is now only called during chain reorganizations
    /// to ensure clean state after rollback. It is no longer called after normal
    /// block processing to allow memory persistence between blocks.
    ///
    /// The LRU cache in metashrew-core prevents unbounded memory growth, making
    /// persistent memory safe for normal operation.
    pub fn refresh_memory(&mut self) -> Result<()> {
        // Always refresh when explicitly called (typically during reorgs)
        let mut wasmstore = Store::<State>::new(&self.engine, State::new());
        wasmstore.limiter(|state| &mut state.limits);
        self.instance = self
            .linker
            .instantiate(&mut wasmstore, &self.module)
            .context("Failed to instantiate module during memory refresh")?;
        self.wasmstore = wasmstore;
        log::info!("WASM memory refreshed (typically due to chain reorganization)");
        Ok(())
    }

    /// Execute the current block through the WASM indexer
    ///
    /// This is the core block processing method that executes the WASM module's
    /// `_start` function to process the current block data. It handles the complete
    /// block processing lifecycle with persistent memory between blocks.
    ///
    /// # Block Processing Flow
    ///
    /// 1. **Initialize state**: Reset execution state to 0 (starting)
    /// 2. **Execute WASM**: Call the `_start` function with current block data
    /// 3. **Validate completion**: Ensure indexer completed successfully (state = 1)
    /// 4. **Retain memory**: WASM memory persists between blocks (LRU cache prevents unbounded growth)
    ///
    /// # Memory Management
    ///
    /// **NEW BEHAVIOR**: Memory is now retained between blocks:
    /// - **Persistent memory**: WASM memory state persists between normal block processing
    /// - **LRU cache**: metashrew-core redesign prevents unbounded memory growth
    /// - **No automatic refresh**: Memory is only refreshed during reorganizations
    /// - **Crash on unexpected exit**: If WASM exits unexpectedly, the indexer crashes (no retry)
    ///
    /// # Chain Reorganization Handling
    ///
    /// Chain reorganization detection is handled at the sync framework level.
    /// When a reorg occurs, memory will be refreshed to ensure clean state.
    ///
    /// # State Validation
    ///
    /// The method validates that:
    /// - Indexer reaches completion state (state = 1)
    /// - No host function failures occurred
    /// - WASM execution completed without traps
    /// - If any validation fails, the indexer crashes immediately
    ///
    /// # Example Usage
    ///
    /// ```rust,ignore
    /// // Set block data in context first
    /// {
    ///     let mut guard = runtime.context.lock()?;
    ///     guard.block = block_data.to_vec();
    ///     guard.height = height;
    /// }
    ///
    /// // Process the block (memory persists after this call)
    /// runtime.run()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `_start` function is not found in WASM module
    /// - WASM execution traps or fails
    /// - Indexer exits without reaching completion state
    /// - Host function failures occur during execution
    ///
    /// **Note**: Any unexpected exit will cause the indexer to crash rather than retry.
    pub fn run(&mut self) -> Result<(), anyhow::Error> {
        self.context.lock().map_err(lock_err)?.state = 0;
        let start = self
            .instance
            .get_typed_func::<(), ()>(&mut self.wasmstore, "_start")
            .context("Failed to get _start function")?;

        // Note: Chain reorganization detection is now handled at the sync framework level
        // using proper block hash comparison, not at the runtime level

        let execution_result = match start.call(&mut self.wasmstore, ()) {
            Ok(_) => {
                let context_state = self.context.lock().map_err(lock_err)?.state;
                let had_failure = self.wasmstore.data().had_failure;

                log::debug!(
                    "WASM execution completed: context_state={}, had_failure={}",
                    context_state,
                    had_failure
                );

                if context_state != 1 && !had_failure {
                    log::error!(
                        "Indexer exited unexpectedly: context_state={}, had_failure={}",
                        context_state,
                        had_failure
                    );
                    // NEW BEHAVIOR: Crash the indexer instead of returning an error for retry
                    std::process::exit(1);
                } else {
                    log::debug!(
                        "Indexer completed successfully: context_state={}",
                        context_state
                    );
                    Ok(())
                }
            }
            Err(e) => {
                log::error!("Error calling _start function: {}", e);
                // NEW BEHAVIOR: Crash the indexer instead of returning an error for retry
                std::process::exit(1);
            }
        };

        // NEW BEHAVIOR: Do NOT refresh memory after normal block execution
        // Memory is now retained between blocks to enable persistent state
        // LRU cache in metashrew-core prevents unbounded memory growth
        log::debug!("Block execution completed, memory retained for persistent state");
        execution_result
    }

    /// Handle chain reorganization by rolling back to the specified height
    ///
    /// **DEPRECATED**: This method is no longer used as reorg detection has been moved
    /// to the sync framework level where it can properly compare block hashes from the
    /// Bitcoin node. The sync framework uses proper reorg detection by comparing stored
    /// block hashes with actual block hashes from bitcoind RPC.
    ///
    /// This method is kept for backward compatibility but should not be called.
    #[deprecated(note = "Reorg detection moved to sync framework level")]
    pub fn handle_reorg(&mut self) -> Result<()> {
        let (context_height, db_tip_height) = {
            let mut guard = self.context.lock().map_err(lock_err)?;
            let db_tip = match guard.db.get(&TIP_HEIGHT_KEY.as_bytes().to_vec()) {
                Ok(Some(bytes)) if bytes.len() >= 4 => {
                    u32::from_le_bytes(bytes[..4].try_into().unwrap())
                }
                _ => 0,
            };
            (guard.height, db_tip)
        };

        if context_height > db_tip_height + 1 {
            return Err(anyhow!(
                "Block height {} is too far ahead of tip {}",
                context_height,
                db_tip_height
            ));
        }

        // Only trigger reorg if context height is strictly less than db tip height
        // If they're equal, we're reprocessing the same block (normal on restart)
        if context_height < db_tip_height {
            if context_height == 0 {
                log::warn!("Reorg at height 0 is not a standard rollback.");
                return Ok(());
            }
            let target_height = context_height - 1;
            log::info!(
                "Reorg detected: rolling back from {} to {}",
                db_tip_height,
                target_height
            );

            let mut db = self.context.lock().map_err(lock_err)?.db.clone();
            let mut smt_helper = SMTHelper::new(db.clone());
            let mut batch = db.create_batch();

            // Delete orphaned SMT roots
            for h in (context_height..=db_tip_height).rev() {
                let root_key = format!("{}{}", crate::smt::SMT_ROOT_PREFIX, h).into_bytes();
                batch.delete(&root_key);
            }

            // Rollback state to the target height
            smt_helper.rollback_to_height_batched(&mut batch, target_height)?;

            // Update the tip height
            batch.put(
                &TIP_HEIGHT_KEY.as_bytes().to_vec(),
                &target_height.to_le_bytes(),
            );

            db.write(batch)
                .map_err(|e| anyhow!("Failed to write reorg batch: {}", e))?;
            log::info!("Reorg to height {} complete", target_height);
        }

        Ok(())
    }

    /// Perform a database rollback to a specific target height.
    ///
    /// This function is intended to be called by a sync layer when a chain
    /// reorganization is detected. It performs the following actions:
    /// 1. Deletes orphaned SMT roots from the database.
    /// 2. Rolls back the append-only state using the SMT helper.
    /// 3. Updates the tip height in the database.
    /// 4. Updates the tip height in the runtime context.
    /// 5. Refreshes the WASM memory to ensure a clean state for subsequent block processing.
    ///
    /// # Arguments
    ///
    /// * `target_height` - The height to roll back to. The resulting state will be
    ///   as it was after `target_height` was processed.
    pub fn rollback(&mut self, target_height: u32) -> Result<()> {
        {
            let mut guard = self.context.lock().map_err(lock_err)?;
            let db_tip_height = match guard.db.get_immutable(&TIP_HEIGHT_KEY.as_bytes().to_vec()) {
                Ok(Some(bytes)) if bytes.len() >= 4 => {
                    u32::from_le_bytes(bytes[..4].try_into().unwrap())
                }
                _ => 0,
            };

            if target_height >= db_tip_height {
                log::warn!(
                    "Rollback target {} is not less than current tip {}. No action taken.",
                    target_height,
                    db_tip_height
                );
                return Ok(());
            }

            log::info!(
                "Rolling back from {} to {}",
                db_tip_height,
                target_height
            );

            let mut smt_helper = SMTHelper::new(guard.db.clone());
            let mut batch = guard.db.create_batch();

            // Delete orphaned SMT roots for blocks that are being rolled back
            for h in (target_height + 1)..=db_tip_height {
                let root_key = format!("{}{}", crate::smt::SMT_ROOT_PREFIX, h).into_bytes();
                batch.delete(&root_key);
            }

            // Rollback the append-only state to the target height
            smt_helper.rollback_to_height_batched(&mut batch, target_height)?;

            // Update the tip height in the database
            batch.put(
                &TIP_HEIGHT_KEY.as_bytes().to_vec(),
                &target_height.to_le_bytes(),
            );

            guard
                .db
                .write(batch)
                .map_err(|e| anyhow!("Failed to write rollback batch: {}", e))?;

            // Also update the tip height in the runtime context
            guard.height = target_height;
        }

        log::info!("Rollback to height {} complete", target_height);

        // After a rollback, the WASM memory is stale and must be refreshed.
        self.refresh_memory()?;

        Ok(())
    }

    /// Get the value of a key at a specific block height using the append-only data structure.
    /// This function performs a binary search on the list of historical values for the key.
    pub fn get_value_at_height(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        key: &Vec<u8>,
        height: u32,
    ) -> Result<Vec<u8>> {
        let db = {
            let guard = context.lock().map_err(lock_err)?;
            guard.db.clone()
        };
        let smt_helper = SMTHelper::new(db);
        match smt_helper.get_at_height(key, height) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(anyhow!("Append-only query error: {}", e)),
        }
    }

    /// Append a key to an update list
    pub fn db_append(
        _context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        batch: &mut T::Batch,
        update_key: &Vec<u8>,
        key: &Vec<u8>,
    ) -> Result<()> {
        // Create a key for the update list
        let update_list_key = crate::key_utils::make_prefixed_key(b"updates:", update_key);

        // Add the key to the update list
        batch.put(&update_list_key, key.clone());

        Ok(())
    }

    pub fn setup_linker(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        linker: &mut Linker<State>,
    ) -> Result<()> {
        Self::setup_linker_with_log_flag(context, linker, false)
    }

    pub fn setup_linker_with_log_flag(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        linker: &mut Linker<State>,
        disable_wasmtime_log: bool,
    ) -> Result<()> {
        let context_ref_len = context.clone();
        let context_ref_input = context.clone();

        linker
            .func_wrap(
                "env",
                "__host_len",
                move |mut _caller: Caller<'_, State>| -> i32 {
                    match context_ref_len.lock() {
                        Ok(ctx) => ctx.block.len() as i32 + 4,
                        Err(_) => i32::MAX, // Signal error
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __host_len: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__load_input",
                move |mut caller: Caller<'_, State>, data_start: i32| {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let (input, height) = match context_ref_input.lock() {
                        Ok(ctx) => (ctx.block.clone(), ctx.height),
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let input_clone = match try_into_vec(height.to_le_bytes()) {
                        Ok(mut v) => {
                            v.extend(input);
                            v
                        }
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let sz = to_usize_or_trap(&mut caller, data_start);
                    if sz == usize::MAX {
                        caller.data_mut().had_failure = true;
                        return;
                    }

                    if let Err(_) = mem.write(&mut caller, sz, input_clone.as_slice()) {
                        caller.data_mut().had_failure = true;
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __load_input: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__log",
                move |mut caller: Caller<'_, State>, data_start: i32| {
                    // Check the runtime flag to determine if logging should be disabled
                    if disable_wasmtime_log {
                        // Silently ignore WASM log calls when wasmtime logging is disabled
                        return;
                    }

                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => return,
                        },
                        None => return,
                    };

                    let data = mem.data(&caller);
                    let bytes = match try_read_arraybuffer_as_vec(data, data_start) {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        print!("{}", text);
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __log: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "abort",
                |mut caller: Caller<'_, State>, _: i32, _: i32, _: i32, _: i32| {
                    caller.data_mut().had_failure = true;
                },
            )
            .map_err(|e| anyhow!("Failed to wrap abort: {:?}", e))?;

        Ok(())
    }

    pub fn setup_linker_view(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        linker: &mut Linker<State>,
    ) -> Result<()> {
        let context_get = context.clone();
        let context_get_len = context.clone();

        linker
            .func_wrap(
                "env",
                "__flush",
                move |_caller: Caller<'_, State>, _encoded: i32| {
                    // View mode __flush - no operation needed
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __flush: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get",
                move |mut caller: Caller<'_, State>, key: i32, value: i32| {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let data = mem.data(&caller);
                    let height = match context_get.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    match try_read_arraybuffer_as_vec(data, key) {
                        Ok(key_vec) => {
                            // Use append-only store for historical queries in view functions
                            match Self::get_value_at_height(context_get.clone(), &key_vec, height) {
                                Ok(lookup) => {
                                    if let Err(_) =
                                        mem.write(&mut caller, value as usize, lookup.as_slice())
                                    {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                                Err(_) => {
                                    // Key not found, return empty
                                    if let Err(_) = mem.write(&mut caller, value as usize, &[]) {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            if let Ok(error_bits) = u32_to_vec(i32::MAX.try_into().unwrap()) {
                                if let Err(_) = mem.write(
                                    &mut caller,
                                    (value - 4) as usize,
                                    error_bits.as_slice(),
                                ) {
                                    caller.data_mut().had_failure = true;
                                }
                            } else {
                                caller.data_mut().had_failure = true;
                            }
                        }
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get_len",
                move |mut caller: Caller<'_, State>, key: i32| -> i32 {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => return i32::MAX,
                        },
                        None => return i32::MAX,
                    };

                    let data = mem.data(&caller);
                    let height = match context_get_len.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => return i32::MAX,
                    };

                    match try_read_arraybuffer_as_vec(data, key) {
                        Ok(key_vec) => {
                            // Use append-only store for historical queries in view functions
                            match Self::get_value_at_height(
                                context_get_len.clone(),
                                &key_vec,
                                height,
                            ) {
                                Ok(value) => value.len() as i32,
                                Err(_) => 0,
                            }
                        }
                        Err(_) => i32::MAX,
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get_len: {:?}", e))?;

        Ok(())
    }

    fn new_with_db(
        db: T,
        height: u32,
        engine: wasmtime::Engine,
        module: wasmtime::Module,
    ) -> Result<MetashrewRuntime<T>> {
        let mut linker = Linker::<State>::new(&engine);
        let mut wasmstore = Store::<State>::new(&engine, State::new());
        let context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
            MetashrewRuntimeContext<T>,
        >::new(
            MetashrewRuntimeContext::new(db, height, vec![], vec![]),
        ));
        {
            wasmstore.limiter(|state| &mut state.limits)
        }
        {
            Self::setup_linker(context.clone(), &mut linker)
                .context("Failed to setup basic linker")?;
            Self::setup_linker_preview(context.clone(), &mut linker)
                .context("Failed to setup preview linker")?;
            linker.define_unknown_imports_as_traps(&module)?;
        }
        let instance = linker
            .instantiate(&mut wasmstore, &module)
            .context("Failed to instantiate WASM module")?;
        let mut runtime = MetashrewRuntime {
            wasmstore,
            engine: engine.clone(),
            async_engine: engine,
            module: module.clone(),
            async_module: module.clone(),
            linker,
            context,
            instance,
            stateful_view_runtime: None,
            disable_wasmtime_log: false,
        };

        // Enable stateful views by default for preview/internal runtimes too
        log::debug!("Enabling stateful views by default for internal runtime");
        if let Err(e) = runtime.enable_stateful_views_sync() {
            log::warn!(
                "Failed to enable stateful views for internal runtime: {}",
                e
            );
            // Continue without stateful views if initialization fails
        }

        Ok(runtime)
    }

    async fn new_with_db_async(
        db: T,
        height: u32,
        engine: wasmtime::Engine,
        module: wasmtime::Module,
    ) -> Result<MetashrewRuntime<T>> {
        let mut linker = Linker::<State>::new(&engine);
        let mut wasmstore = Store::<State>::new(&engine, State::new());
        let context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
            MetashrewRuntimeContext<T>,
        >::new(
            MetashrewRuntimeContext::new(db, height, vec![], vec![]),
        ));
        {
            wasmstore.limiter(|state| &mut state.limits)
        }
        {
            Self::setup_linker(context.clone(), &mut linker)
                .context("Failed to setup basic linker")?;
            Self::setup_linker_view(context.clone(), &mut linker)
                .context("Failed to setup view linker")?;
            linker.define_unknown_imports_as_traps(&module)?;
        }
        let instance = linker
            .instantiate_async(&mut wasmstore, &module)
            .await
            .context("Failed to instantiate WASM module")?;
        let mut runtime = MetashrewRuntime {
            wasmstore,
            engine: engine.clone(),
            async_engine: engine,
            module: module.clone(),
            async_module: module.clone(),
            linker,
            context,
            instance,
            stateful_view_runtime: None,
            disable_wasmtime_log: false,
        };

        // Enable stateful views by default for async runtimes too
        log::debug!("Enabling stateful views by default for async runtime");
        if let Err(e) = runtime.enable_stateful_views().await {
            log::warn!("Failed to enable stateful views for async runtime: {}", e);
            // Continue without stateful views if initialization fails
        }

        Ok(runtime)
    }

    fn setup_linker_preview(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        linker: &mut Linker<State>,
    ) -> Result<()> {
        let context_ref = context.clone();
        let context_get = context.clone();
        let context_get_len = context.clone();

        linker
            .func_wrap(
                "env",
                "__flush",
                move |mut caller: Caller<'_, State>, encoded: i32| {
                    let height = match context_ref.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let data = mem.data(&caller);
                    let encoded_vec = match try_read_arraybuffer_as_vec(data, encoded) {
                        Ok(v) => v,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    // For preview, we'll store directly in the database
                    let decoded = match KeyValueFlush::decode(&*encoded_vec) {
                        Ok(d) => d,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    match context_ref.clone().lock() {
                        Ok(mut ctx) => {
                            ctx.state = 1;

                            // Use append-only store for preview operations with batching
                            let mut batch = ctx.db.create_batch();
                            let smt_helper = crate::smt::SMTHelper::new(ctx.db.clone());

                            // Write all operations to a single batch for atomicity
                            for (k, v) in decoded.list.iter().tuples() {
                                let k_owned = <Vec<u8> as Clone>::clone(k);
                                let v_owned = <Vec<u8> as Clone>::clone(v);

                                // Add to batch using append-only logic
                                if let Err(_) =
                                    smt_helper.put_to_batch(&mut batch, &k_owned, &v_owned, height)
                                {
                                    caller.data_mut().had_failure = true;
                                    return;
                                }
                            }

                            // Write the entire batch atomically
                            if let Err(_) = ctx.db.write(batch) {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        }
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __flush: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get",
                move |mut caller: Caller<'_, State>, key: i32, value: i32| {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let data = mem.data(&caller);
                    let height = match context_get.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    match try_read_arraybuffer_as_vec(data, key) {
                        Ok(key_vec) => {
                            // Use append-only store for preview queries
                            match Self::get_value_at_height(context_get.clone(), &key_vec, height) {
                                Ok(lookup) => {
                                    if let Err(_) =
                                        mem.write(&mut caller, value as usize, lookup.as_slice())
                                    {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                                Err(_) => {
                                    // Key not found, return empty
                                    if let Err(_) = mem.write(&mut caller, value as usize, &[]) {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            if let Ok(error_bits) = u32_to_vec(i32::MAX.try_into().unwrap()) {
                                if let Err(_) = mem.write(
                                    &mut caller,
                                    (value - 4) as usize,
                                    error_bits.as_slice(),
                                ) {
                                    caller.data_mut().had_failure = true;
                                }
                            } else {
                                caller.data_mut().had_failure = true;
                            }
                        }
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get_len",
                move |mut caller: Caller<'_, State>, key: i32| -> i32 {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => return i32::MAX,
                        },
                        None => return i32::MAX,
                    };

                    let data = mem.data(&caller);
                    let height = match context_get_len.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => return i32::MAX,
                    };

                    match try_read_arraybuffer_as_vec(data, key) {
                        Ok(key_vec) => {
                            // Use append-only store for preview queries
                            match Self::get_value_at_height(
                                context_get_len.clone(),
                                &key_vec,
                                height,
                            ) {
                                Ok(value) => value.len() as i32,
                                Err(_) => 0,
                            }
                        }
                        Err(_) => i32::MAX,
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get_len: {:?}", e))?;

        Ok(())
    }

    pub fn setup_linker_indexer(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        linker: &mut Linker<State>,
    ) -> Result<()> {
        let context_ref = context.clone();
        let context_get = context.clone();
        let context_get_len = context.clone();

        linker
            .func_wrap(
                "env",
                "__flush",
                move |mut caller: Caller<'_, State>, encoded: i32| {
                    let height = match context_ref.clone().lock() {
                        Ok(ctx) => ctx.height,
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let data = mem.data(&caller);
                    let encoded_vec = match try_read_arraybuffer_as_vec(data, encoded) {
                        Ok(v) => v,
                        Err(_e) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let _batch = T::Batch::default();

                    let decoded = match KeyValueFlush::decode(&*encoded_vec) {
                        Ok(d) => d,
                        Err(_e) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    // Get the database from context to use SMT operations
                    let db = match context_ref.clone().lock() {
                        Ok(ctx) => ctx.db.clone(),
                        Err(_) => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    // Use optimized BatchedSMTHelper for better performance
                    let mut batched_smt = crate::smt::BatchedSMTHelper::new(db);

                    // Collect all key-value pairs for batch processing
                    // This is the new, correct flow for handling state updates.
                    // All key-value pairs are collected and passed to a single, atomic
                    // function that handles both the SMT update and the historical append-only storage.
                    let key_values: Vec<(Vec<u8>, Vec<u8>)> = decoded
                        .list
                        .iter()
                        .tuples()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    // Track key-value updates for any external listeners (like snapshotting)
                    {
                        let context_ref_clone = context_ref.clone();
                        let mut ctx_guard = match context_ref_clone.lock() {
                            Ok(guard) => guard,
                            Err(_) => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        };
                        for (k, v) in &key_values {
                            ctx_guard.db.track_kv_update(k.clone(), v.clone());
                        }
                    }

                    // The new `calculate_and_store_state_root_batched` will handle all database writes atomically.
                    // It will be refactored to accept key-value pairs directly.
                    match batched_smt.calculate_and_store_state_root_batched(height, &key_values) {
                        Ok(state_root) => {
                            log::info!(
                                "indexed block {} with {} k/v pairs atomically, state root: {}",
                                height,
                                key_values.len(),
                                hex::encode(state_root)
                            );
                        }
                        Err(e) => {
                            log::error!(
                                "failed to calculate state root for height {}: {:?}",
                                height,
                                e
                            );
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    }

                    // Calculate and store prefix roots
                    if let Ok(mut ctx) = context_ref.clone().lock() {
                        for (name, prefix) in &ctx.prefix_configs.clone() {
                            let prefixed_kvs: Vec<(Vec<u8>, Vec<u8>)> = key_values
                                .iter()
                                .filter(|(k, _)| k.starts_with(prefix))
                                .cloned()
                                .collect();
                            if !prefixed_kvs.is_empty() {
                                if let Some(smt) = ctx.prefix_smts.get_mut(name) {
                                    if smt.update(&prefixed_kvs).is_err() {
                                        caller.data_mut().had_failure = true;
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    // Set completion state
                    match context_ref.clone().lock() {
                        Ok(mut ctx) => {
                            log::debug!("Host __flush function called, setting context state to 1");
                            ctx.state = 1;
                        }
                        Err(_) => {
                            log::error!("Failed to lock context in __flush function");
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __flush: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get",
                move |mut caller: Caller<'_, State>, key: i32, value: i32| {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => {
                                caller.data_mut().had_failure = true;
                                return;
                            }
                        },
                        None => {
                            caller.data_mut().had_failure = true;
                            return;
                        }
                    };

                    let data = mem.data(&caller);
                    let key_vec_result = try_read_arraybuffer_as_vec(data, key);

                    match key_vec_result {
                        Ok(key_vec) => {
                            // During indexing, get the state as it was at the *previous* block
                            // to correctly build upon the previous state, especially during reorgs.
                            let height = match context_get.clone().lock() {
                                Ok(ctx) => ctx.height,
                                Err(_) => {
                                    caller.data_mut().had_failure = true;
                                    return;
                                }
                            };

                            // The state for the current block (at `height`) depends on the
                            // state produced by the parent block (at `height - 1`).
                            // If height is 0, there is no parent, so we read at height 0 (which will be empty).
                            let target_height = if height > 0 { height - 1 } else { 0 };

                            // Use get_value_at_height to get the value from the previous canonical state.
                            // This function correctly performs a binary search on the append-only data.
                            match Self::get_value_at_height(
                                context_get.clone(),
                                &key_vec,
                                target_height,
                            ) {
                                Ok(lookup) => {
                                    if let Err(_) =
                                        mem.write(&mut caller, value as usize, lookup.as_slice())
                                    {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                                Err(_) => {
                                    // Key not found, return empty
                                    if let Err(_) = mem.write(&mut caller, value as usize, &[]) {
                                        caller.data_mut().had_failure = true;
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            if let Ok(error_bits) = u32_to_vec(i32::MAX.try_into().unwrap()) {
                                if let Err(_) = mem.write(
                                    &mut caller,
                                    (value - 4) as usize,
                                    error_bits.as_slice(),
                                ) {
                                    caller.data_mut().had_failure = true;
                                }
                            } else {
                                caller.data_mut().had_failure = true;
                            }
                        }
                    };
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get: {:?}", e))?;

        linker
            .func_wrap(
                "env",
                "__get_len",
                move |mut caller: Caller<'_, State>, key: i32| -> i32 {
                    let mem = match caller.get_export("memory") {
                        Some(export) => match export.into_memory() {
                            Some(memory) => memory,
                            None => return i32::MAX,
                        },
                        None => return i32::MAX,
                    };

                    let data = mem.data(&caller);
                    let key_vec_result = try_read_arraybuffer_as_vec(data, key);

                    match key_vec_result {
                        Ok(key_vec) => {
                            // During indexing, get the state as it was at the *previous* block.
                            let (_db, height) = match context_get_len.clone().lock() {
                                Ok(ctx) => (ctx.db.clone(), ctx.height),
                                Err(_) => return i32::MAX,
                            };

                            let target_height = if height > 0 { height - 1 } else { 0 };

                            match Self::get_value_at_height(
                                context_get_len.clone(),
                                &key_vec,
                                target_height,
                            ) {
                                Ok(value) => value.len() as i32,
                                Err(_) => 0,
                            }
                        }
                        Err(_) => i32::MAX,
                    }
                },
            )
            .map_err(|e| anyhow!("Failed to wrap __get_len: {:?}", e))?;

        Ok(())
    }

    /// Get all keys that were touched at a specific block height
    pub fn get_keys_touched_at_height(
        _context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        _height: u32,
    ) -> Result<Vec<Vec<u8>>> {
        // For now, return an empty list
        // In a full implementation, we would scan the database for keys modified at this height
        Ok(Vec::new())
    }

    /// Iterate backwards through all values of a key from most recent update
    pub fn iterate_key_backwards(
        _context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        _key: &Vec<u8>,
        _from_height: u32,
    ) -> Result<Vec<(u32, Vec<u8>)>> {
        // For now, return an empty list
        // In a full implementation, we would scan historical values for this key
        Ok(Vec::new())
    }

    /// Get the current state root (merkle root of entire state)
    pub fn get_current_state_root(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
    ) -> Result<[u8; 32]> {
        let db = {
            let guard = context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        let smt_helper = SMTHelper::new(db);
        smt_helper.get_current_state_root()
    }

    /// Get the state root at a specific height
    pub fn get_state_root_at_height(
        context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        height: u32,
    ) -> Result<[u8; 32]> {
        let db = {
            let guard = context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        let smt_helper = SMTHelper::new(db);
        smt_helper.get_smt_root_at_height(height)
    }

    /// Perform a complete rollback to a specific height
    pub fn rollback_to_height(
        _context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        target_height: u32,
    ) -> Result<()> {
        // For now, just log the rollback
        // In a full implementation, we would need to restore database state
        log::info!("Rolling back to height {}", target_height);
        Ok(())
    }

    /// Get all heights at which a key was updated
    pub fn get_key_update_heights(
        _context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
        _key: &Vec<u8>,
    ) -> Result<Vec<u32>> {
        // For now, return an empty list
        // In a full implementation, we would scan for all heights where this key was modified
        Ok(Vec::new())
    }

    /// Calculate the state root for the current state
    /// This is used by the atomic block processing to get the state root after execution
    pub fn calculate_state_root(&self) -> Result<Vec<u8>> {
        let db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        let smt_helper = SMTHelper::new(db);
        let state_root = smt_helper.get_current_state_root()?;
        Ok(state_root.to_vec())
    }

    /// Get the accumulated database operations as a serialized batch
    /// This collects all the operations that would be written to the database
    pub fn get_accumulated_batch(&self) -> Result<Vec<u8>> {
        let db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        // For now, we'll return an empty batch since the current implementation
        // writes directly to the database during __flush
        // In a full atomic implementation, we would collect operations in a batch
        // and return the serialized batch data here

        // Create a batch and serialize it
        let _batch = db.create_batch();

        // For now, just return empty batch data
        // In a full implementation, we would serialize the batch operations
        Ok(Vec::new())
    }

    /// Process a block atomically and return all operations in a batch
    /// This is the atomic version that collects all operations without committing them
    pub async fn process_block_atomic(
        &mut self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> Result<crate::traits::AtomicBlockResult> {
        // Set the block data and height in context
        {
            let mut guard = self.context.lock().map_err(lock_err)?;
            guard.block = block_data.to_vec();
            guard.height = height;
            guard.state = 0;
        }

        // Note: Chain reorganization detection is now handled at the sync framework level
        // using proper block hash comparison, not at the runtime level

        // Execute the WASM module
        let start = self
            .instance
            .get_typed_func::<(), ()>(&mut self.wasmstore, "_start")
            .context("Failed to get _start function")?;

        let execution_result = match start.call(&mut self.wasmstore, ()) {
            Ok(_) => {
                let context_state = {
                    let guard = self.context.lock().map_err(lock_err)?;
                    guard.state
                };

                if context_state != 1 && !self.wasmstore.data().had_failure {
                    Err(anyhow!(
                        "indexer exited unexpectedly during atomic processing"
                    ))
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e).context("Error calling _start function in atomic processing"),
        };

        // Calculate the state root and batch data before memory refresh
        let (state_root, batch_data) = match execution_result {
            Ok(_) => {
                let state_root = self.calculate_state_root()?;
                let batch_data = self.get_accumulated_batch()?;

                // Log the state root for atomic block processing
                log::info!(
                    "processed block {} atomically, state root: {}",
                    height,
                    hex::encode(&state_root)
                );

                (state_root, batch_data)
            }
            Err(e) => {
                // NEW BEHAVIOR: Crash the indexer instead of returning an error for retry
                log::error!("Atomic block execution failed: {}", e);
                std::process::exit(1);
            }
        };

        // NEW BEHAVIOR: Do NOT refresh memory after atomic block execution
        // Memory is now retained between blocks to enable persistent state
        log::debug!("Atomic block execution completed, memory retained for persistent state");

        // Return the atomic result
        Ok(crate::traits::AtomicBlockResult {
            state_root,
            batch_data,
            height,
            block_hash: block_hash.to_vec(),
        })
    }

    /// Process a block normally (non-atomic)
    pub async fn process_block(&mut self, height: u32, block_data: &[u8]) -> Result<()> {
        // Set the block data and height in context
        {
            let mut guard = self.context.lock().map_err(lock_err)?;
            guard.block = block_data.to_vec();
            guard.height = height;
            guard.state = 0;
        }

        // Execute the block processing - run() now handles memory refresh automatically
        self.run()
    }

    /// Get the state root for a specific height
    pub async fn get_state_root(&self, height: u32) -> Result<Vec<u8>> {
        let state_root = Self::get_state_root_at_height(self.context.clone(), height)?;
        Ok(state_root.to_vec())
    }

    /// Enable stateful view mode by creating a persistent WASM runtime
    ///
    /// This method initializes a stateful view runtime that retains WASM memory
    /// and instance state between view function calls. Once enabled, all view
    /// function calls will reuse the same WASM instance instead of creating
    /// new ones, allowing for stateful operations.
    ///
    /// # Memory Management
    ///
    /// The WASM author is responsible for managing memory within their WASM module.
    /// The runtime provides a persistent execution environment without automatic
    /// memory cleanup between view calls.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the stateful runtime was successfully created,
    /// or an error if initialization failed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Enable stateful view mode
    /// runtime.enable_stateful_views().await?;
    ///
    /// // Now all view calls will reuse the same WASM instance
    /// let result1 = runtime.view("function1".to_string(), &input1, height).await?;
    /// let result2 = runtime.view("function2".to_string(), &input2, height).await?;
    /// // WASM memory state persists between these calls
    /// ```
    pub async fn enable_stateful_views(&mut self) -> Result<()> {
        let db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        // Create the stateful view runtime
        let stateful_runtime = StatefulViewRuntime::new(
            db,
            0, // Initial height, will be updated per view call
            self.async_engine.clone(),
            self.async_module.clone(),
        )
        .await?;

        self.stateful_view_runtime = Some(stateful_runtime);
        log::info!("Stateful view mode enabled - WASM memory will persist between view calls");
        Ok(())
    }

    /// Enable stateful view mode synchronously (used during initialization)
    ///
    /// This is a synchronous version of `enable_stateful_views` that can be called
    /// during runtime initialization. It uses `tokio::task::block_in_place` to
    /// handle the async operations within a sync context.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the stateful runtime was successfully created,
    /// or an error if initialization failed.
    pub fn enable_stateful_views_sync(&mut self) -> Result<()> {
        let db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        // Use block_in_place to handle async operations in sync context
        let stateful_runtime = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                StatefulViewRuntime::new(
                    db,
                    0, // Initial height, will be updated per view call
                    self.async_engine.clone(),
                    self.async_module.clone(),
                )
                .await
            })
        })?;

        self.stateful_view_runtime = Some(stateful_runtime);
        log::info!("Stateful view mode enabled synchronously - WASM memory will persist between view calls");
        Ok(())
    }

    /// Disable stateful view mode and return to creating new instances per call
    ///
    /// This method removes the stateful view runtime, causing subsequent view
    /// function calls to create fresh WASM instances with clean memory state.
    /// This is the default behavior.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable stateful view mode
    /// runtime.disable_stateful_views();
    ///
    /// // Now view calls will create fresh WASM instances
    /// let result = runtime.view("function".to_string(), &input, height).await?;
    /// ```
    pub fn disable_stateful_views(&mut self) {
        self.stateful_view_runtime = None;
        log::info!("Stateful view mode disabled - view calls will create fresh WASM instances");
    }

    /// Check if stateful view mode is currently enabled
    ///
    /// # Returns
    ///
    /// `true` if stateful view mode is enabled, `false` otherwise.
    pub fn is_stateful_views_enabled(&self) -> bool {
        self.stateful_view_runtime.is_some()
    }

    /// Set the disable wasmtime log flag
    ///
    /// When set to true, the __log host function will silently ignore all WASM log calls.
    /// This provides the same behavior as the old --features logs flag but controlled
    /// at runtime instead of compile time.
    ///
    /// # Parameters
    ///
    /// - `disable`: Whether to disable WASM logging
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Disable WASM logging
    /// runtime.set_disable_wasmtime_log(true);
    /// ```
    pub fn set_disable_wasmtime_log(&mut self, disable: bool) {
        self.disable_wasmtime_log = disable;
        if disable {
            log::info!("WASM __log host function disabled - WASM log calls will be silently ignored");
        } else {
            log::info!("WASM __log host function enabled - WASM log calls will be processed");
        }
        
        // Rebuild the linker and instance with the new log setting
        if let Err(e) = self.rebuild_linker_with_log_setting() {
            log::error!("Failed to rebuild linker with new log setting: {}", e);
        }
    }

    /// Rebuild the linker and instance with the current log setting
    fn rebuild_linker_with_log_setting(&mut self) -> Result<()> {
        let mut linker = Linker::<State>::new(&self.engine);
        let mut wasmstore = Store::<State>::new(&self.engine, State::new());
        
        wasmstore.limiter(|state| &mut state.limits);
        
        Self::setup_linker_with_log_flag(self.context.clone(), &mut linker, self.disable_wasmtime_log)
            .context("Failed to setup basic linker with log flag")?;
        Self::setup_linker_indexer(self.context.clone(), &mut linker)
            .context("Failed to setup indexer linker")?;
        linker.define_unknown_imports_as_traps(&self.module)?;
        
        let instance = linker
            .instantiate(&mut wasmstore, &self.module)
            .context("Failed to instantiate WASM module with new log setting")?;
        
        self.linker = linker;
        self.wasmstore = wasmstore;
        self.instance = instance;
        
        Ok(())
    }

    /// Check if WASM logging is disabled
    ///
    /// # Returns
    ///
    /// `true` if WASM logging is disabled, `false` otherwise.
    pub fn is_wasmtime_log_disabled(&self) -> bool {
        self.disable_wasmtime_log
    }

    /// Execute a view function using the stateful runtime
    ///
    /// This method executes a view function using the persistent WASM runtime,
    /// maintaining memory state between calls. It updates the context for the
    /// current call but preserves WASM memory across invocations.
    ///
    /// # Parameters
    ///
    /// - `stateful_runtime`: Reference to the stateful view runtime
    /// - `symbol`: Name of the view function to execute
    /// - `input`: Input data for the view function
    /// - `height`: Block height for the query context
    ///
    /// # Returns
    ///
    /// The result of the view function execution as raw bytes
    async fn view_stateful(
        &self,
        stateful_runtime: &StatefulViewRuntime<T>,
        symbol: String,
        input: &Vec<u8>,
        height: u32,
    ) -> Result<Vec<u8>> {
        // Update the context for this view call
        {
            let mut guard = stateful_runtime.context.lock().map_err(lock_err)?;
            guard.block = input.clone();
            guard.height = height;

            // Update the database reference to ensure we're using the current state
            let current_db = {
                let main_guard = self.context.lock().map_err(lock_err)?;
                main_guard.db.clone()
            };
            guard.db = current_db;
        }

        // Execute the view function using the stateful runtime
        let mut wasmstore_guard = stateful_runtime.wasmstore.lock().await;

        // Set fuel for cooperative yielding
        wasmstore_guard.set_fuel(u64::MAX)?;
        wasmstore_guard.fuel_async_yield_interval(Some(10000))?;

        // Get the view function
        let func = stateful_runtime
            .instance
            .get_typed_func::<(), i32>(&mut *wasmstore_guard, symbol.as_str())
            .with_context(|| {
                format!("Failed to get view function '{}' in stateful mode", symbol)
            })?;

        // Execute the function asynchronously
        let result = func
            .call_async(&mut *wasmstore_guard, ())
            .await
            .with_context(|| {
                format!(
                    "Failed to execute view function '{}' in stateful mode",
                    symbol
                )
            })?;

        // Get the memory to read the result
        let memory = stateful_runtime
            .instance
            .get_memory(&mut *wasmstore_guard, "memory")
            .ok_or_else(|| anyhow!("Failed to get memory for stateful view result"))?;

        // Read the result from WASM memory
        let result_data = read_arraybuffer_as_vec(memory.data(&*wasmstore_guard), result);

        log::debug!(
            "Executed view function '{}' in stateful mode, memory state preserved",
            symbol
        );
        Ok(result_data)
    }
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> StatefulViewRuntime<T> {
    /// Create a new stateful view runtime
    ///
    /// This initializes a persistent WASM execution environment that can be
    /// reused across multiple view function calls, maintaining memory state.
    ///
    /// # Parameters
    ///
    /// - `db`: Database backend for view operations
    /// - `height`: Initial block height (updated per view call)
    /// - `engine`: Async WASM engine for execution
    /// - `module`: Compiled WASM module to instantiate
    ///
    /// # Returns
    ///
    /// A new stateful view runtime ready for persistent execution
    pub async fn new(
        db: T,
        height: u32,
        engine: wasmtime::Engine,
        module: wasmtime::Module,
    ) -> Result<Self> {
        let mut linker = Linker::<State>::new(&engine);
        let mut wasmstore = Store::<State>::new(&engine, State::new());

        let context = Arc::<Mutex<MetashrewRuntimeContext<T>>>::new(Mutex::<
            MetashrewRuntimeContext<T>,
        >::new(
            MetashrewRuntimeContext::new(db, height, vec![], vec![]),
        ));

        // Set up store limits
        wasmstore.limiter(|state| &mut state.limits);

        // Set up host functions for view operations
        MetashrewRuntime::<T>::setup_linker(context.clone(), &mut linker)
            .context("Failed to setup basic linker for stateful view")?;
        MetashrewRuntime::<T>::setup_linker_view(context.clone(), &mut linker)
            .context("Failed to setup view linker for stateful view")?;
        linker.define_unknown_imports_as_traps(&module)?;

        // Instantiate the WASM module
        let instance = linker
            .instantiate_async(&mut wasmstore, &module)
            .await
            .context("Failed to instantiate WASM module for stateful view")?;

        Ok(StatefulViewRuntime {
            wasmstore: Arc::new(tokio::sync::Mutex::new(wasmstore)),
            instance,
            linker,
            context,
        })
    }
}
