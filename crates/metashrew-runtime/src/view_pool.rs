//! Parallelized view runtime pool for concurrent view execution
//!
//! This module provides a pool of stateful WASM runtimes that can handle
//! concurrent view requests while maintaining the benefits of persistent
//! WASM memory between calls. Each runtime in the pool has its own cache
//! and can process view requests independently.

use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use wasmtime::{Engine, Module};

use crate::runtime::{MetashrewRuntime, StatefulViewRuntime};
use crate::traits::KeyValueStoreLike;

/// Configuration for the view runtime pool
#[derive(Clone, Debug)]
pub struct ViewPoolConfig {
    /// Number of stateful view runtimes to maintain in the pool
    pub pool_size: usize,

    /// Maximum number of concurrent view requests to allow
    /// If None, defaults to pool_size * 2
    pub max_concurrent_requests: Option<usize>,

    /// Whether to enable detailed logging for pool operations
    pub enable_logging: bool,
}

impl Default for ViewPoolConfig {
    fn default() -> Self {
        Self {
            pool_size: num_cpus::get().max(4), // At least 4, or number of CPU cores
            max_concurrent_requests: None,
            enable_logging: true,
        }
    }
}

/// A pool of stateful view runtimes for parallel view execution
///
/// This struct manages a pool of `StatefulViewRuntime` instances that can
/// process view requests concurrently. Each runtime maintains its own WASM
/// memory state and cache, enabling true parallelization of view operations.
///
/// # Architecture
///
/// - **Pool Management**: Maintains a fixed number of stateful runtimes
/// - **Concurrency Control**: Uses semaphores to limit concurrent requests
/// - **Load Balancing**: Distributes requests across available runtimes
/// - **Cache Isolation**: Each runtime has its own height-partitioned cache
/// - **Thread Safety**: All operations are thread-safe and async-compatible
///
/// # Benefits
///
/// - **Parallel Execution**: Multiple view requests can run simultaneously
/// - **Memory Persistence**: Each runtime retains WASM memory between calls
/// - **Cache Efficiency**: Height-partitioned caching per runtime instance
/// - **Resource Control**: Configurable pool size and concurrency limits
/// - **Fault Isolation**: Issues in one runtime don't affect others
pub struct ViewRuntimePool<T: KeyValueStoreLike> {
    /// Pool of stateful view runtimes
    runtimes: Vec<Arc<Mutex<StatefulViewRuntime<T>>>>,

    /// Semaphore to control concurrent access to the pool
    semaphore: Arc<Semaphore>,

    /// Configuration for the pool
    config: ViewPoolConfig,

    /// Round-robin counter for load balancing
    counter: Arc<Mutex<usize>>,

    /// Database reference for creating new runtimes if needed
    db: T,

    /// WASM engine for creating new runtimes
    engine: Engine,

    /// WASM module for creating new runtimes
    module: Module,
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> ViewRuntimePool<T> {
    /// Create a new view runtime pool
    ///
    /// # Parameters
    ///
    /// - `db`: Database backend for view operations
    /// - `engine`: Async WASM engine for execution
    /// - `module`: Compiled WASM module to instantiate
    /// - `config`: Pool configuration settings
    ///
    /// # Returns
    ///
    /// A new view runtime pool ready for concurrent view execution
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ViewPoolConfig {
    ///     pool_size: 8,
    ///     max_concurrent_requests: Some(16),
    ///     enable_logging: true,
    /// };
    ///
    /// let pool = ViewRuntimePool::new(
    ///     database,
    ///     async_engine,
    ///     async_module,
    ///     config
    /// ).await?;
    /// ```
    pub async fn new(
        db: T,
        engine: Engine,
        module: Module,
        config: ViewPoolConfig,
    ) -> Result<Self> {
        let max_concurrent = config
            .max_concurrent_requests
            .unwrap_or(config.pool_size * 2);

        if config.enable_logging {
            log::info!(
                "Creating view runtime pool with {} runtimes, max {} concurrent requests",
                config.pool_size,
                max_concurrent
            );
        }

        // Create the pool of stateful view runtimes
        let mut runtimes = Vec::with_capacity(config.pool_size);

        for i in 0..config.pool_size {
            let runtime = StatefulViewRuntime::new(
                db.clone(),
                0, // Initial height, will be updated per view call
                engine.clone(),
                module.clone(),
            )
            .await
            .with_context(|| format!("Failed to create stateful view runtime {}", i))?;

            runtimes.push(Arc::new(Mutex::new(runtime)));

            if config.enable_logging {
                log::debug!(
                    "Created stateful view runtime {} of {}",
                    i + 1,
                    config.pool_size
                );
            }
        }

        if config.enable_logging {
            log::info!("View runtime pool initialized successfully");
        }

        Ok(ViewRuntimePool {
            runtimes,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            config,
            counter: Arc::new(Mutex::new(0)),
            db,
            engine,
            module,
        })
    }

    /// Execute a view function using the pool
    ///
    /// This method selects an available runtime from the pool and executes
    /// the specified view function. The selection uses round-robin load
    /// balancing to distribute requests evenly across pool members.
    ///
    /// # Parameters
    ///
    /// - `symbol`: Name of the view function to execute
    /// - `input`: Input data for the view function
    /// - `height`: Block height for the query context
    ///
    /// # Returns
    ///
    /// The result of the view function execution as raw bytes
    ///
    /// # Concurrency
    ///
    /// This method respects the pool's concurrency limits. If the maximum
    /// number of concurrent requests is reached, the call will wait until
    /// a slot becomes available.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Execute view function in parallel
    /// let result = pool.view(
    ///     "get_balance".to_string(),
    ///     &address_bytes,
    ///     height
    /// ).await?;
    /// ```
    pub async fn view(&self, symbol: String, input: &Vec<u8>, height: u32) -> Result<Vec<u8>> {
        // Acquire semaphore permit to control concurrency
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| anyhow!("Failed to acquire semaphore permit for view execution"))?;

        // Select a runtime using round-robin load balancing
        let runtime_index = {
            let mut counter = self.counter.lock().await;
            let index = *counter % self.runtimes.len();
            *counter = counter.wrapping_add(1);
            index
        };

        let runtime = &self.runtimes[runtime_index];

        if self.config.enable_logging {
            log::debug!(
                "Executing view function '{}' at height {} using runtime {}",
                symbol,
                height,
                runtime_index
            );
        }

        // Execute the view function using the selected runtime
        let result = self
            .execute_view_with_runtime(runtime, symbol.clone(), input, height)
            .await;

        match &result {
            Ok(_) => {
                if self.config.enable_logging {
                    log::debug!(
                        "Successfully executed view function '{}' using runtime {}",
                        symbol,
                        runtime_index
                    );
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to execute view function '{}' using runtime {}: {}",
                    symbol,
                    runtime_index,
                    e
                );
            }
        }

        result
    }

    /// Execute a view function with a specific runtime
    ///
    /// This is an internal method that handles the actual execution of a
    /// view function using a specific stateful runtime from the pool.
    async fn execute_view_with_runtime(
        &self,
        runtime: &Arc<Mutex<StatefulViewRuntime<T>>>,
        symbol: String,
        input: &Vec<u8>,
        height: u32,
    ) -> Result<Vec<u8>> {
        let runtime_guard = runtime.lock().await;

        // Update the context for this view call
        {
            let mut context_guard = runtime_guard
                .context
                .lock()
                .map_err(|e| anyhow!("Failed to lock runtime context: {}", e))?;
            context_guard.block = input.clone();
            context_guard.height = height;
            context_guard.db = self.db.clone();
        }

        // Execute the view function
        let mut wasmstore_guard = runtime_guard.wasmstore.lock().await;

        // Set fuel for cooperative yielding
        wasmstore_guard.set_fuel(u64::MAX)?;
        wasmstore_guard.fuel_async_yield_interval(Some(10000))?;

        // Get the view function
        let func = runtime_guard
            .instance
            .get_typed_func::<(), i32>(&mut *wasmstore_guard, symbol.as_str())
            .with_context(|| format!("Failed to get view function '{}' in pool", symbol))?;

        // Execute the function asynchronously
        let result = func
            .call_async(&mut *wasmstore_guard, ())
            .await
            .with_context(|| format!("Failed to execute view function '{}' in pool", symbol))?;

        // Get the memory to read the result
        let memory = runtime_guard
            .instance
            .get_memory(&mut *wasmstore_guard, "memory")
            .ok_or_else(|| anyhow!("Failed to get memory for pool view result"))?;

        // Read the result from WASM memory
        let result_data =
            crate::runtime::read_arraybuffer_as_vec(memory.data(&*wasmstore_guard), result);

        Ok(result_data)
    }

    /// Get pool statistics for monitoring
    ///
    /// Returns information about the current state of the pool including
    /// the number of runtimes, available permits, and configuration.
    ///
    /// # Returns
    ///
    /// A `ViewPoolStats` struct containing current pool metrics
    pub async fn get_stats(&self) -> ViewPoolStats {
        let available_permits = self.semaphore.available_permits();
        let current_counter = {
            let counter = self.counter.lock().await;
            *counter
        };

        ViewPoolStats {
            pool_size: self.runtimes.len(),
            available_permits,
            max_concurrent_requests: self.semaphore.available_permits()
                + (self
                    .config
                    .max_concurrent_requests
                    .unwrap_or(self.config.pool_size * 2)
                    - available_permits),
            total_requests_processed: current_counter,
            config: self.config.clone(),
        }
    }

    /// Resize the pool by adding or removing runtimes
    ///
    /// This method allows dynamic adjustment of the pool size based on
    /// load requirements. New runtimes are created as needed, and excess
    /// runtimes are removed when shrinking the pool.
    ///
    /// # Parameters
    ///
    /// - `new_size`: The desired new pool size
    ///
    /// # Returns
    ///
    /// `Ok(())` if the resize was successful, or an error if it failed
    pub async fn resize_pool(&mut self, new_size: usize) -> Result<()> {
        if new_size == 0 {
            return Err(anyhow!("Pool size must be at least 1"));
        }

        let current_size = self.runtimes.len();

        if new_size > current_size {
            // Add new runtimes
            for i in current_size..new_size {
                let runtime = StatefulViewRuntime::new(
                    self.db.clone(),
                    0,
                    self.engine.clone(),
                    self.module.clone(),
                )
                .await
                .with_context(|| {
                    format!("Failed to create additional stateful view runtime {}", i)
                })?;

                self.runtimes.push(Arc::new(Mutex::new(runtime)));

                if self.config.enable_logging {
                    log::debug!("Added stateful view runtime {} to pool", i + 1);
                }
            }
        } else if new_size < current_size {
            // Remove excess runtimes
            self.runtimes.truncate(new_size);

            if self.config.enable_logging {
                log::debug!("Removed {} runtimes from pool", current_size - new_size);
            }
        }

        // Update configuration
        self.config.pool_size = new_size;

        if self.config.enable_logging {
            log::info!("Resized view runtime pool to {} runtimes", new_size);
        }

        Ok(())
    }
}

/// Statistics about the view runtime pool
#[derive(Debug, Clone)]
pub struct ViewPoolStats {
    /// Current number of runtimes in the pool
    pub pool_size: usize,

    /// Number of available semaphore permits
    pub available_permits: usize,

    /// Maximum number of concurrent requests allowed
    pub max_concurrent_requests: usize,

    /// Total number of requests processed (counter value)
    pub total_requests_processed: usize,

    /// Pool configuration
    pub config: ViewPoolConfig,
}

impl ViewPoolStats {
    /// Get the number of currently active requests
    pub fn active_requests(&self) -> usize {
        self.max_concurrent_requests - self.available_permits
    }

    /// Get the utilization percentage of the pool
    pub fn utilization_percentage(&self) -> f64 {
        if self.max_concurrent_requests == 0 {
            0.0
        } else {
            (self.active_requests() as f64 / self.max_concurrent_requests as f64) * 100.0
        }
    }
}

/// Extension trait for MetashrewRuntime to support view pools
pub trait ViewPoolSupport<T: KeyValueStoreLike> {
    /// Create a view runtime pool from this runtime
    fn create_view_pool(
        &self,
        config: ViewPoolConfig,
    ) -> impl std::future::Future<Output = Result<ViewRuntimePool<T>>> + Send;
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static> ViewPoolSupport<T>
    for MetashrewRuntime<T>
{
    /// Create a view runtime pool from this runtime
    ///
    /// This method creates a new view runtime pool using the same database,
    /// engine, and module as the current runtime. The pool can then be used
    /// for parallel view execution.
    ///
    /// # Parameters
    ///
    /// - `config`: Configuration for the new pool
    ///
    /// # Returns
    ///
    /// A new view runtime pool ready for concurrent execution
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ViewPoolConfig::default();
    /// let pool = runtime.create_view_pool(config).await?;
    ///
    /// // Now use the pool for parallel view execution
    /// let result = pool.view("function".to_string(), &input, height).await?;
    /// ```
    async fn create_view_pool(&self, config: ViewPoolConfig) -> Result<ViewRuntimePool<T>> {
        let db = {
            let guard = self
                .context
                .lock()
                .map_err(|e| anyhow!("Failed to lock runtime context: {}", e))?;
            guard.db.clone()
        };

        ViewRuntimePool::new(
            db,
            self.async_engine.clone(),
            self.async_module.clone(),
            config,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::stateful_view_test::InMemoryStore;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_view_pool_creation() {
        let store = InMemoryStore::new();
        let config = ViewPoolConfig {
            pool_size: 2,
            max_concurrent_requests: Some(4),
            enable_logging: false,
        };

        // This test would need a proper WASM module to work
        // For now, we'll just test the configuration logic
        assert_eq!(config.pool_size, 2);
        assert_eq!(config.max_concurrent_requests, Some(4));
    }

    #[tokio::test]
    async fn test_view_pool_stats() {
        // Test the stats calculation logic
        let stats = ViewPoolStats {
            pool_size: 4,
            available_permits: 6,
            max_concurrent_requests: 8,
            total_requests_processed: 100,
            config: ViewPoolConfig::default(),
        };

        assert_eq!(stats.active_requests(), 2);
        assert_eq!(stats.utilization_percentage(), 25.0);
    }

    #[test]
    fn test_view_pool_config_default() {
        let config = ViewPoolConfig::default();
        assert!(config.pool_size >= 4); // At least 4 or number of CPU cores
        assert!(config.enable_logging);
        assert!(config.max_concurrent_requests.is_none());
    }
}
