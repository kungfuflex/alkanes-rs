use crate::vm::errors::{IndexerError, IndexerResult};
use crate::vm::validation::MemoryValidator;
use hex;
use wasmi::*;
use std::convert::TryFrom;

pub struct SafeMemory<'a> {
    memory: &'a Memory,
    validator: &'a MemoryValidator,
}

impl<'a> SafeMemory<'a> {
    pub fn new(memory: &'a Memory, validator: &'a MemoryValidator) -> Self {
        Self { memory, validator }
    }

    pub fn read_bytes<T: 'a>(&self, store: impl Into<StoreContext<'a, T>>, offset: i32, len: usize) -> IndexerResult<Vec<u8>> {
        self.validator.validate_memory_access(self.memory, offset, len)?;

        let offset_usize = usize::try_from(offset).map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid offset conversion: {}", e))
        })?;

        let data = self.memory.data(store);
        let mut buffer = vec![0u8; len];
        buffer.copy_from_slice(
            data.get(offset_usize..offset_usize + len)
                .ok_or_else(|| {
                    IndexerError::MemoryAccess("Memory region out of bounds".to_string())
                })?,
        );

        Ok(buffer)
    }

    pub fn write_bytes<T: 'a>(&self, mut store: impl Into<StoreContextMut<'a, T>>, offset: i32, data: &[u8]) -> IndexerResult<()> {
        self.validator.validate_memory_access(self.memory, offset, data.len())?;

        let offset_usize = usize::try_from(offset).map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid offset conversion: {}", e))
        })?;

        self.memory
            .write(&mut store, offset_usize, data)
            .map_err(|_| IndexerError::MemoryAccess("Failed to write memory".to_string()))?;

        Ok(())
    }

    pub fn read_arraybuffer(&self, ptr: i32) -> IndexerResult<Vec<u8>> {
        // First 4 bytes contain the length
        let len_bytes = self.read_bytes(ptr, 4)?;
        let len = u32::from_le_bytes(len_bytes.try_into().map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid length conversion: {}", hex::encode(e)))
        })?) as usize;

        // Read the actual data
        self.read_bytes(ptr + 4, len)
    }

    pub fn write_arraybuffer(&self, ptr: i32, data: &[u8]) -> IndexerResult<i32> {
        // Write length first
        let len_bytes = (data.len() as u32).to_le_bytes();
        self.write_bytes(ptr, &len_bytes)?;

        // Write actual data
        self.write_bytes(ptr + 4, data)?;

        Ok(ptr + 4)
    }
}

/// Safe wrapper for memory operations in host functions
pub struct HostMemoryContext<'a> {
    memory: SafeMemory<'a>,
}

impl<'a> HostMemoryContext<'a> {
    pub fn new(memory: &'a Memory, validator: &'a MemoryValidator) -> Self {
        Self {
            memory: SafeMemory::new(memory, validator),
        }
    }

    pub fn read_arraybuffer(&self, ptr: i32) -> IndexerResult<Vec<u8>> {
        self.memory.read_arraybuffer(ptr)
    }

    pub fn write_arraybuffer(&self, ptr: i32, data: &[u8]) -> IndexerResult<i32> {
        self.memory.write_arraybuffer(ptr, data)
    }
}
