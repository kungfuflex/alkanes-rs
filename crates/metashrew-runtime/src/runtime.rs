//! Core Native runtime for executing Bitcoin indexers
//
// This module provides the main [`MetashrewRuntime`] struct that executes
// native Rust code for Bitcoin block processing. It no longer uses WebAssembly.

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;

use crate::context::MetashrewRuntimeContext;
use crate::smt::SMTHelper;
use crate::traits::{BatchLike, KeyValueStoreLike};
use metashrew_core::indexer::Indexer;
use metashrew_core::{get_cache, get_to_flush, initialize, clear};
use bitcoin::Block;

/// Internal key used to store the current blockchain tip height
///
/// This key is used internally by the runtime to track the highest block
/// that has been successfully processed and committed to the database.
pub const TIP_HEIGHT_KEY: &'static str = "/__INTERNAL/tip-height";

fn lock_err<T>(err: std::sync::PoisonError<T>) -> anyhow::Error {
    anyhow!("Mutex lock error: {}", err)
}


/// Core native runtime for executing Bitcoin indexers
///
/// [`MetashrewRuntime`] is the main execution engine that runs native Rust indexer
/// logic for Bitcoin block processing. It's generic over storage backends, enabling
/// flexible deployment scenarios from testing to production.
///
/// # Type Parameters
///
/// - `T`: Storage backend implementing [`KeyValueStoreLike`] + [`Clone`] + [`Send`] + [`Sync`]
///
pub struct MetashrewRuntime<T: KeyValueStoreLike, I: Indexer> {
    /// Shared execution context containing database, block data, and state
    ///
    /// Protected by [`Arc<Mutex<_>>`] for thread-safe access across
    /// different execution modes and concurrent view operations.
    pub context: Arc<Mutex<MetashrewRuntimeContext<T>>>,
    _indexer: PhantomData<I>,
}

impl<T: KeyValueStoreLike + Clone + Send + Sync + 'static, I: Indexer + Default> MetashrewRuntime<T, I> {
    pub fn new(
        mut store: T,
        prefix_configs: Vec<(String, Vec<u8>)>,
    ) -> Result<Self> {
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
        
        Ok(MetashrewRuntime {
            context,
            _indexer: PhantomData,
        })
    }

    pub fn process_block(&mut self, height: u32, block: &Block) -> Result<()> {
        // Initialize metashrew-core's global caches
        initialize();

        // Set the block data and height in context
        {
            let mut guard = self.context.lock().map_err(lock_err)?;
            guard.height = height;
        }

        // Run the native indexer logic
        let indexer = I::default();
        indexer.index_block(block, height)?;

        // Flush the results to the database
        self.flush(height)?;

        // Clear the caches for the next block
        clear();

        Ok(())
    }

    fn flush(&mut self, height: u32) -> Result<()> {
        let mut db = {
            let guard = self.context.lock().map_err(lock_err)?;
            guard.db.clone()
        };

        let smt_helper = SMTHelper::new(db.clone());
        let mut batch = db.create_batch();

        let cache = get_cache();
        let to_flush = get_to_flush();

        for key in to_flush.iter() {
            if let Some(value) = cache.get(&**key) {
                smt_helper.put_to_batch(&mut batch, key, value, height)?;
            }
        }
        
        db.write(batch)?;

        Ok(())
    }

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

        Ok(())
    }
}
