use crate::vm::errors::{IndexerError, IndexerResult};
use crate::vm::validation::MemoryValidator;
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

    pub fn read_bytes<T>(&self, store: &impl AsContext<Data = T>, offset: i32, len: usize) -> IndexerResult<Vec<u8>> {
        self.validator.validate_memory_access(self.memory, offset, len)?;

        let offset_usize = usize::try_from(offset).map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid offset conversion: {:?}", e))
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

    pub fn write_bytes<T>(&self, store: &mut impl AsContextMut<Data = T>, offset: i32, data: &[u8]) -> IndexerResult<()> {
        self.validator.validate_memory_access(self.memory, offset, data.len())?;

        let offset_usize = usize::try_from(offset).map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid offset conversion: {:?}", e))
        })?;

        self.memory
            .write(store, offset_usize, data)
            .map_err(|_| IndexerError::MemoryAccess("Failed to write memory".to_string()))?;

        Ok(())
    }

    pub fn read_arraybuffer<T>(&self, store: &impl AsContext<Data = T>, ptr: i32) -> IndexerResult<Vec<u8>> {
        // First 4 bytes contain the length
        let len_bytes = self.read_bytes(store, ptr, 4)?;
        let len = u32::from_le_bytes(len_bytes.try_into().map_err(|e| {
            IndexerError::IntegerConversion(format!("Invalid length conversion: {:?}", e))
        })?) as usize;

        // Read the actual data
        self.read_bytes(store, ptr + 4, len)
    }

    pub fn write_arraybuffer<T>(&self, store: &mut impl AsContextMut<Data = T>, ptr: i32, data: &[u8]) -> IndexerResult<i32> {
        // Write length first
        self.write_bytes(store, ptr, &(data.len() as u32).to_le_bytes())?;

        // Write actual data
        self.write_bytes(store, ptr + 4, data)?;

        Ok(ptr + 4)
    }
}

pub struct HostMemoryContext<'a> {
    memory: SafeMemory<'a>,
}

impl<'a> HostMemoryContext<'a> {
    pub fn new(memory: &'a Memory, validator: &'a MemoryValidator) -> Self {
        Self {
            memory: SafeMemory::new(memory, validator),
        }
    }

    pub fn read_arraybuffer<T>(&self, store: &impl AsContext<Data = T>, ptr: i32) -> IndexerResult<Vec<u8>> {
        self.memory.read_arraybuffer(store, ptr)
    }

    pub fn write_arraybuffer<T>(&self, store: &mut impl AsContextMut<Data = T>, ptr: i32, data: &[u8]) -> IndexerResult<i32> {
        self.memory.write_arraybuffer(store, ptr, data)
    }
}
