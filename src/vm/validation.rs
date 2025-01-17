use crate::vm::errors::{IndexerError, IndexerResult, MAX_MEMORY_SIZE, MAX_WASM_SIZE};
use wasmi::*;

#[derive(Clone)]
pub struct MemoryValidator {
    max_size: usize,
}

impl MemoryValidator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }

    pub fn validate_memory_access(&self, _: &Memory, start: i32, len: usize) -> IndexerResult<()> {
        if start < 0 {
            return Err(IndexerError::MemoryAccess(
                "Negative memory offset".to_string(),
            ));
        }

        let start_usize = start as usize;
        if start_usize + len > self.max_size {
            return Err(IndexerError::MemoryAccess(format!(
                "Memory access out of bounds: start={}, len={}, max={}",
                start_usize, len, self.max_size
            )));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct WasmValidator {
    max_size: usize,
    required_exports: Vec<String>,
}

impl WasmValidator {
    pub fn new(max_size: usize, required_exports: Vec<String>) -> Self {
        Self {
            max_size,
            required_exports,
        }
    }

    pub fn validate_module(&self, binary: &[u8], engine: &Engine) -> IndexerResult<Module> {
        // Check module size
        if binary.len() > self.max_size {
            return Err(IndexerError::WasmValidation(format!(
                "WASM module too large: {} bytes (max: {})",
                binary.len(),
                self.max_size
            )));
        }

        // Parse and validate the module
        let module = Module::new(engine, binary).map_err(|e| {
            IndexerError::WasmValidation(format!("Failed to parse WASM module: {}", e))
        })?;

        // Validate required exports
        for export in &self.required_exports {
            if !module.exports().any(|e| e.name() == export) {
                return Err(IndexerError::ExportValidation(format!(
                    "Missing required export: {}",
                    export
                )));
            }
        }

        Ok(module)
    }
}

#[derive(Default, Clone)]
pub struct ResourceTracker {
    memory_usage: usize,
    instruction_count: u64,
    error_count: u64,
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self {
            memory_usage: 0,
            instruction_count: 0,
            error_count: 0,
        }
    }

    pub fn track_memory_allocation(&mut self, size: usize) -> IndexerResult<()> {
        let new_total = self.memory_usage.checked_add(size).ok_or_else(|| {
            IndexerError::ResourceExhausted("Memory usage would overflow".to_string())
        })?;

        if new_total > MAX_MEMORY_SIZE {
            return Err(IndexerError::ResourceExhausted(format!(
                "Memory limit exceeded: {} (max: {})",
                new_total, MAX_MEMORY_SIZE
            )));
        }

        self.memory_usage = new_total;
        Ok(())
    }

    pub fn track_instruction(&mut self, count: u64) -> IndexerResult<()> {
        self.instruction_count = self.instruction_count.checked_add(count).ok_or_else(|| {
            IndexerError::ResourceExhausted("Instruction count would overflow".to_string())
        })?;
        Ok(())
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }
}

#[derive(Clone)]
pub struct ValidationLayer {
    pub memory_validator: MemoryValidator,
    pub wasm_validator: WasmValidator,
    pub resource_tracker: ResourceTracker,
}

impl Default for ValidationLayer {
    fn default() -> Self {
        Self {
            memory_validator: MemoryValidator::new(MAX_MEMORY_SIZE),
            wasm_validator: WasmValidator::new(
                MAX_WASM_SIZE,
                vec!["memory".to_string(), "execute".to_string()],
            ),
            resource_tracker: ResourceTracker::new(),
        }
    }
}

pub struct ResourceGuard<T> {
    pub resource: T,
    cleanup: Option<Box<dyn FnMut(&mut T)>>,
}

impl<T> ResourceGuard<T> {
    pub fn new(resource: T, cleanup: Box<dyn FnMut(&mut T)>) -> Self {
        Self { 
            resource, 
            cleanup: Some(cleanup)
        }
    }
}

impl<T> Default for ResourceGuard<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            resource: T::default(),
            cleanup: None,
        }
    }
}

impl<T> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if let Some(mut cleanup) = self.cleanup.take() {
            cleanup(&mut self.resource);
        }
    }
}

/// Helper trait for guarding resources
pub trait GuardedResource: Sized {
    fn with_guard<F>(self, cleanup: F) -> ResourceGuard<Self>
    where
        F: FnMut(&mut Self) + 'static,
    {
        ResourceGuard::new(self, Box::new(cleanup))
    }
}
