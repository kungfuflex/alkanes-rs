//! Runtime execution context for WebAssembly indexers
//!
//! This module provides the [`MetashrewRuntimeContext`] struct that maintains
//! the execution state and environment for WASM indexer modules. The context
//! is shared between the host runtime and WASM modules through host functions.
//!
//! # Architecture
//!
//! The context serves as the bridge between:
//! - **Host runtime**: Manages block processing and database operations
//! - **WASM modules**: Execute indexing logic with access to context data
//! - **Storage backend**: Provides persistent state through generic interface
//!
//! # Thread Safety
//!
//! The context is protected by `Arc<Mutex<_>>` in the runtime to enable
//! safe concurrent access across different execution modes and host functions.
//!
//! # Lifecycle
//!
//! 1. **Initialization**: Context created with storage backend and initial state
//! 2. **Block setup**: Block data and height set before WASM execution
//! 3. **Execution**: WASM module accesses context through host functions
//! 4. **Completion**: State updated to reflect successful processing
//! 5. **Cleanup**: Context prepared for next block or operation

use crate::smt::BatchedSMTHelper;
use crate::traits::KeyValueStoreLike;
use std::collections::HashMap;

/// Execution context for WebAssembly indexer modules
///
/// [`MetashrewRuntimeContext`] maintains the complete execution environment
/// for WASM indexers, including database access, block data, and execution state.
/// It's designed to be generic over storage backends for maximum flexibility.
///
/// # Type Parameters
///
/// - `T`: Storage backend implementing [`KeyValueStoreLike`]
///
/// # Fields
///
/// ## Database Access
/// - `db`: Storage backend for persistent state management
///
/// ## Block Context
/// - `height`: Current block height being processed
/// - `block`: Raw block data available to WASM modules
///
/// ## Execution State
/// - `state`: Tracks WASM execution progress and completion
///
/// # State Values
///
/// The `state` field tracks execution progress:
/// - `0`: Execution starting/in progress
/// - `1`: Execution completed successfully
/// - Other values may indicate specific error conditions
///
/// # Usage Pattern
///
/// ```rust,ignore
/// // Create context with storage backend
/// let context = MetashrewRuntimeContext::new(
///     storage_backend,
///     block_height,
///     block_data
/// );
///
/// // Context is typically wrapped in Arc<Mutex<_>> for thread safety
/// let shared_context = Arc::new(Mutex::new(context));
/// ```
///
/// # Thread Safety
///
/// While the context itself is not thread-safe, it's designed to be used
/// within `Arc<Mutex<_>>` for safe concurrent access across host functions
/// and different execution modes.
pub struct MetashrewRuntimeContext<T: KeyValueStoreLike> {
    /// Storage backend for persistent state management
    ///
    /// Provides access to the key-value database where indexed data is stored.
    /// The generic design allows different storage implementations (RocksDB,
    /// in-memory, etc.) to be used based on deployment requirements.
    pub db: T,

    /// Current block height being processed
    ///
    /// This height is used for:
    /// - Height-indexed storage operations
    /// - Historical state queries
    /// - Chain reorganization detection
    /// - State root calculations
    pub height: u32,

    /// Raw block data available to WASM modules
    ///
    /// Contains the complete block data that WASM indexers can access through
    /// host functions. The format depends on the specific blockchain and
    /// indexer requirements.
    pub block: Vec<u8>,

    /// WASM execution state tracking
    ///
    /// Tracks the progress of WASM module execution:
    /// - `0`: Execution starting or in progress
    /// - `1`: Execution completed successfully
    /// - Other values may indicate error conditions
    pub state: u32,

    /// Configurations for prefix-based SMT roots
    pub prefix_configs: Vec<(String, Vec<u8>)>,

    /// Calculated SMT roots for each configured prefix
    pub prefix_smts: HashMap<String, BatchedSMTHelper<T>>,
}

impl<T: KeyValueStoreLike> Clone for MetashrewRuntimeContext<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            height: self.height,
            block: self.block.clone(),
            state: self.state,
            prefix_configs: self.prefix_configs.clone(),
            prefix_smts: self.prefix_smts.clone(),
        }
    }
}

impl<T: KeyValueStoreLike + Clone> MetashrewRuntimeContext<T> {
    /// Create a new runtime context with the specified parameters
    ///
    /// Initializes a new execution context for WASM indexer modules with
    /// the provided storage backend, block height, and block data.
    ///
    /// # Parameters
    ///
    /// - `db`: Storage backend implementing [`KeyValueStoreLike`]
    /// - `height`: Block height for this execution context
    /// - `block`: Raw block data to be processed
    ///
    /// # Returns
    ///
    /// A new [`MetashrewRuntimeContext`] with execution state set to 0 (starting)
    ///
    /// # Initial State
    ///
    /// The context is created with:
    /// - Execution state set to 0 (starting/in progress)
    /// - All provided parameters stored for WASM access
    /// - Ready for use in WASM runtime execution
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let context = MetashrewRuntimeContext::new(
    ///     my_storage_backend,
    ///     block_height,
    ///     block_data_bytes
    /// );
    ///
    /// // Context is ready for WASM execution
    /// assert_eq!(context.state, 0); // Starting state
    /// assert_eq!(context.height, block_height);
    /// ```
    ///
    /// # Usage in Runtime
    ///
    /// Typically used within the runtime like:
    /// ```rust,ignore
    /// let context = Arc::new(Mutex::new(
    ///     MetashrewRuntimeContext::new(storage, height, block_data)
    /// ));
    /// ```
    pub fn new(db: T, height: u32, block: Vec<u8>, prefix_configs: Vec<(String, Vec<u8>)>) -> Self {
        let mut prefix_smts = HashMap::new();
        for (name, _) in prefix_configs.iter() {
            prefix_smts.insert(name.clone(), BatchedSMTHelper::new(db.clone()));
        }
        Self {
            db,
            height,
            block,
            state: 0,
            prefix_configs,
            prefix_smts,
        }
    }
}
