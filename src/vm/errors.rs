
use thiserror::Error;
use wasmi::errors::LinkerError;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error("Memory access error: {0}")]
    MemoryAccess(String),
    
    #[error("WASM validation error: {0}")]
    WasmValidation(String),
    
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Mutex lock error: {0}")]
    MutexLock(String),
    
    #[error("Integer conversion error: {0}")]
    IntegerConversion(String),
    
    #[error("Fuel error: {0}")]
    Fuel(String),
    
    #[error("Export validation error: {0}")]
    ExportValidation(String),
    
    #[error("External call error: {0}")]
    ExternalCall(String),
}

pub type IndexerResult<T> = Result<T, IndexerError>;

/// Helper function to convert mutex poison errors to IndexerError
pub fn lock_error<T>(e: std::sync::PoisonError<T>) -> IndexerError {
    IndexerError::MutexLock(e.to_string())
}

/// Trait to convert errors to IndexerError
pub trait IntoIndexerError {
    fn into_indexer_error(self) -> IndexerError;
}

impl<E> IntoIndexerError for E 
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_indexer_error(self) -> IndexerError {
        IndexerError::ExternalCall(self.to_string())
    }
}

impl From<wasmi::Error> for IndexerError {
    fn from(err: wasmi::Error) -> Self {
        IndexerError::WasmValidation(err.to_string())
    }
}

impl From<LinkerError> for IndexerError {
    fn from(err: LinkerError) -> Self {
        IndexerError::ExternalCall(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for IndexerError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        IndexerError::MutexLock(err.to_string())
    }
}

/// Constants for validation
pub const MAX_WASM_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const MAX_MEMORY_SIZE: usize = 32 * 1024 * 1024; // 32MB
pub const MAX_TABLE_SIZE: usize = 65536; // 64K elements
