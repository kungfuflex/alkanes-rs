//! OYL Protocol Integration Module
//! 
//! This module provides comprehensive view functions and data access for the OYL (Oil) protocol,
//! a decentralized exchange built on the alkanes metaprotocol.
//! 
//! The module is organized into several submodules:
//! - `storage`: Storage access patterns and table definitions
//! - `view`: View functions for querying OYL data
//! - `indexer`: Real-time indexing of OYL protocol activities
//! - `utils`: Utility functions for data processing and calculations

pub mod storage;
pub mod view;
pub mod indexer;
pub mod utils;

// Re-export commonly used types
pub use alkanes_support::proto::oyl::*;
pub use storage::*;
pub use view::*;
pub use utils::*;

/// OYL Protocol Constants
pub mod constants {
    use alkanes_support::id::AlkaneId;
    
    /// The hardcoded factory constant for OYL protocol detection
    /// This should be replaced with the actual factory constant when deployed
    pub const OYL_FACTORY_CONSTANT: u128 = 0x4f594c5f464143544f5259; // "OYL_FACTORY" in hex
    
    /// The OYL factory AlkaneId
    pub const OYL_FACTORY_ID: AlkaneId = AlkaneId { 
        block: 2, 
        tx: OYL_FACTORY_CONSTANT 
    };
    
    /// Default fuel amount for view function calls
    pub const DEFAULT_VIEW_FUEL: u64 = 100_000;
    
    /// Maximum number of items to return in paginated responses
    pub const MAX_PAGE_SIZE: u32 = 1000;
    
    /// Default page size for queries
    pub const DEFAULT_PAGE_SIZE: u32 = 50;
    
    /// Time intervals for historical data
    pub const HOUR_SECONDS: u64 = 3600;
    pub const DAY_SECONDS: u64 = 86400;
    pub const WEEK_SECONDS: u64 = 604800;
    pub const MONTH_SECONDS: u64 = 2592000; // 30 days
    
    /// Opcodes for OYL contracts
    pub mod opcodes {
        /// Token contract opcodes
        pub const TOKEN_GET_NAME: u128 = 99;
        pub const TOKEN_GET_SYMBOL: u128 = 100;
        pub const TOKEN_GET_TOTAL_SUPPLY: u128 = 101;
        pub const TOKEN_GET_DATA: u128 = 1000;
        
        /// Factory contract opcodes
        pub const FACTORY_INIT: u128 = 0;
        pub const FACTORY_CREATE_POOL: u128 = 1;
        pub const FACTORY_FIND_POOL: u128 = 2;
        pub const FACTORY_GET_ALL_POOLS: u128 = 3;
        pub const FACTORY_GET_NUM_POOLS: u128 = 4;
        pub const FACTORY_SET_POOL_FACTORY_ID: u128 = 7;
        pub const FACTORY_COLLECT_FEES: u128 = 10;
        pub const FACTORY_SWAP_ALONG_PATH: u128 = 20;
        
        /// Pool contract opcodes
        pub const POOL_INIT: u128 = 0;
        pub const POOL_ADD_LIQUIDITY: u128 = 1;
        pub const POOL_BURN: u128 = 2;
        pub const POOL_SWAP_EXACT_FOR_TOKENS: u128 = 3;
        pub const POOL_SWAP_TOKENS_FOR_EXACT: u128 = 4;
        pub const POOL_COLLECT_FEES: u128 = 10;
        pub const POOL_SWAP: u128 = 20;
        pub const POOL_FORWARD_INCOMING: u128 = 50;
        pub const POOL_GET_NAME: u128 = 99;
        pub const POOL_GET_DETAILS: u128 = 999;
    }
}

/// Error types for OYL operations
#[derive(Debug)]
pub enum OylError {
    TokenNotFound(AlkaneId),
    PoolNotFound(AlkaneId),
    InvalidRequest(String),
    OpcodeCallFailed(String),
    StorageError(String),
    SerializationError(String),
    PriceCalculationError(String),
    PaginationError(String),
}

impl std::fmt::Display for OylError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OylError::TokenNotFound(id) => write!(f, "Token not found: {:?}", id),
            OylError::PoolNotFound(id) => write!(f, "Pool not found: {:?}", id),
            OylError::InvalidRequest(msg) => write!(f, "Invalid request parameters: {}", msg),
            OylError::OpcodeCallFailed(msg) => write!(f, "Opcode call failed: {}", msg),
            OylError::StorageError(msg) => write!(f, "Storage access error: {}", msg),
            OylError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            OylError::PriceCalculationError(msg) => write!(f, "Price calculation error: {}", msg),
            OylError::PaginationError(msg) => write!(f, "Pagination error: {}", msg),
        }
    }
}

impl std::error::Error for OylError {}

pub type OylResult<T> = Result<T, OylError>;

use alkanes_support::id::AlkaneId;