use crate::vm::errors::{IndexerError, IndexerResult};
use crate::vm::memory::HostMemoryContext;
use crate::vm::state_safe::AlkanesStateSafe;
use wasmi::*;
use metashrew_support::index_pointer::{KeyValuePointer};
use std::convert::TryFrom;

pub struct AlkanesHostFunctionsSafe;

impl AlkanesHostFunctionsSafe {
    pub fn abort(mut caller: Caller<'_, AlkanesStateSafe>) {
        caller.data_mut().set_failure();
    }

    pub fn request_storage(caller: &mut Caller<'_, AlkanesStateSafe>, k: i32) -> IndexerResult<i32> {
        let key = {
            let memory = caller.get_export("memory")
                .and_then(|ext| ext.into_memory())
                .ok_or_else(|| IndexerError::MemoryAccess("Failed to get memory export".to_string()))?;
            
            let ctx = HostMemoryContext::new(&memory, &caller.data().validation.memory_validator);
            ctx.read_arraybuffer(caller, k)?
        };

        let data = caller.data();
        let context = data.get_context()?;
        let myself = context.myself.clone();
        let len = context
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key)
            .get()
            .len();

        caller.data().track_instruction(1)?;
        
        i32::try_from(len).map_err(|e| {
            IndexerError::IntegerConversion(format!("Length conversion failed: {}", e))
        })
    }

    pub fn load_storage(
        caller: &mut Caller<'_, AlkanesStateSafe>,
        k: i32,
        v: i32,
    ) -> IndexerResult<i32> {
        let key = {
            let memory = caller.get_export("memory")
                .and_then(|ext| ext.into_memory())
                .ok_or_else(|| IndexerError::MemoryAccess("Failed to get memory export".to_string()))?;
            
            let ctx = HostMemoryContext::new(&memory, &caller.data().validation.memory_validator);
            ctx.read_arraybuffer(caller, k)?
        };

        let context = caller.data().get_context()?;
        let myself = context.myself.clone();
        let value = context
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key)
            .get();

        caller.data().track_instruction(1)?;
        caller.data().track_memory(value.len())?;

        let memory = caller.get_export("memory")
            .and_then(|ext| ext.into_memory())
            .ok_or_else(|| IndexerError::MemoryAccess("Failed to get memory export".to_string()))?;
        
        let ctx = HostMemoryContext::new(&memory, &caller.data().validation.memory_validator);
        ctx.write_arraybuffer(caller, v, &value)
    }

    // TODO: Implement other host functions similarly
}
