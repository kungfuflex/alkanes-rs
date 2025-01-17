use crate::vm::errors::{IndexerError, IndexerResult};
use crate::vm::state_safe::AlkanesStateSafe;
use crate::vm::validation::ValidationLayer;
use crate::vm::AlkanesRuntimeContext;
use crate::vm::host_functions_safe::AlkanesHostFunctionsSafe;
use crate::vm::errors::MAX_MEMORY_SIZE;


use alkanes_support::response::ExtendedCallResponse;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesExecutor {
    instance: Instance,
    store: Store<AlkanesStateSafe>,
    validation: ValidationLayer,
}

impl AlkanesExecutor {
    pub fn new(
        binary: &[u8],
        context: Arc<Mutex<AlkanesRuntimeContext>>,
        start_fuel: u64,
    ) -> IndexerResult<Self> {
        let validation = ValidationLayer::default();
        
        // Configure engine with limits
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);

        // Validate and instantiate module
        let module = validation.wasm_validator.validate_module(binary, &engine)?;

        // Create store with limits
        let mut store = Store::new(
            &engine,
            AlkanesStateSafe::new(
                context,
                StoreLimitsBuilder::new()
                    .memory_size(MAX_MEMORY_SIZE)
                    .build(),
                validation.clone(),
            ),
        );

        // Set initial fuel
        store.set_fuel(start_fuel).map_err(|e| {
            IndexerError::Fuel(format!("Failed to set initial fuel: {}", e))
        })?;

        // Create and initialize linker with safe host functions
        let mut linker = Linker::new(&engine);
        Self::initialize_linker(&mut linker)?;

        // Instantiate module
        let instance = linker
            .instantiate(&mut store, &module)?
            .ensure_no_start(&mut store)
            .map_err(|e| {
                IndexerError::WasmValidation(format!(
                    "Module start function not allowed: {}", 
                    e
                ))
            })?;

        Ok(Self {
            instance,
            store,
            validation,
        })
    }

    fn initialize_linker(linker: &mut Linker<AlkanesStateSafe>) -> IndexerResult<()> {
        // Safe wrapper for host functions
        linker.func_wrap(
            "env", 
            "abort",
            AlkanesHostFunctionsSafe::abort
        )?;

        // Add other safe host functions...

        Ok(())
    }

    pub fn execute(&mut self) -> IndexerResult<ExtendedCallResponse> {
        // Create checkpoint for rollback
        self.store
            .data_mut()
            .get_context()?
            .message
            .atomic
            .checkpoint();

        let result = self.execute_with_recovery();

        match result {
            Ok(response) => {
                if self.store.data().had_failure {
                    self.rollback()?;
                    Err(IndexerError::ExternalCall(
                        "Execution failed with error flag set".to_string()
                    ))
                } else {
                    self.commit()?;
                    Ok(response)
                }
            }
            Err(e) => {
                self.rollback()?;
                Err(e)
            }
        }
    }

    fn execute_with_recovery(&mut self) -> IndexerResult<ExtendedCallResponse> {
        // Reset error state
        self.store.data_mut().had_failure = false;

        // Get execute function
        let execute = self
            .instance
            .get_func(&mut self.store, "execute")
            .ok_or_else(|| {
                IndexerError::ExportValidation(
                    "Missing execute function".to_string()
                )
            })?;

        // Execute with resource tracking
        let result = execute.call(&mut self.store, &[], &mut []);

        match result {
            Ok(_) => {
                let context = self.store.data().get_context()?;
                let response = ExtendedCallResponse::from_context(&context.to_context());
                Ok(response)
            }
            Err(e) => {
                self.store.data_mut().set_failure();
                Err(IndexerError::ExternalCall(format!(
                    "Execution failed: {}", 
                    e
                )))
            }
        }
    }

    fn commit(&mut self) -> IndexerResult<()> {
        self.store
            .data_mut()
            .get_context()?
            .message
            .atomic
            .commit();
        Ok(())
    }

    fn rollback(&mut self) -> IndexerResult<()> {
        self.store
            .data_mut()
            .get_context()?
            .message
            .atomic
            .rollback();
        Ok(())
    }
}
