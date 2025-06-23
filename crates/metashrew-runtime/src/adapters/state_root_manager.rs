//! Generic state root management implementation

use anyhow::Result;
use async_trait::async_trait;
use crate::{KeyValueStoreLike, smt::SMTHelper};
use super::traits::StateRootManager;

/// Generic state root manager that works with any KeyValueStoreLike implementation
pub struct GenericStateRootManager<T: KeyValueStoreLike> {
    storage: T,
}

impl<T: KeyValueStoreLike> GenericStateRootManager<T> {
    pub fn new(storage: T) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl<T: KeyValueStoreLike + Send + Sync + Clone> StateRootManager for GenericStateRootManager<T>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    async fn store_state_root(&self, height: u32, root: &[u8]) -> Result<()> {
        let mut smt_helper = SMTHelper::new(self.storage.clone());
        let root_key = format!("smt:root:{}", height).into_bytes();
        
        smt_helper.storage.put(&root_key, root)
            .map_err(|e| anyhow::anyhow!("Failed to store state root: {}", e))?;
        
        Ok(())
    }

    async fn get_state_root(&self, height: u32) -> Result<Option<Vec<u8>>> {
        let smt_helper = SMTHelper::new(self.storage.clone());
        
        match smt_helper.get_smt_root_at_height(height) {
            Ok(root) => Ok(Some(root.to_vec())),
            Err(_) => Ok(None), // No state root found for this height
        }
    }

    async fn get_latest_state_root(&self) -> Result<Option<(u32, Vec<u8>)>> {
        // This is a simplified implementation - in practice, you might want to
        // maintain an index of the latest height with a state root
        let smt_helper = SMTHelper::new(self.storage.clone());
        
        // Try to find the latest state root by scanning backwards from a reasonable height
        // This is not the most efficient approach, but works for the generic case
        for height in (0..=1000000).rev() {
            if let Ok(root) = smt_helper.get_smt_root_at_height(height) {
                return Ok(Some((height, root.to_vec())));
            }
        }
        
        Ok(None)
    }
}

/// Utility function to get state root from any storage backend
pub async fn get_state_root_at_height<T: KeyValueStoreLike + Clone>(
    storage: &T, 
    height: u32
) -> Result<Option<Vec<u8>>>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let smt_helper = SMTHelper::new(storage.clone());
    
    match smt_helper.get_smt_root_at_height(height) {
        Ok(root) => Ok(Some(root.to_vec())),
        Err(_) => Ok(None),
    }
}

/// Utility function to store state root in any storage backend
pub async fn store_state_root_at_height<T: KeyValueStoreLike + Clone>(
    storage: &T,
    height: u32,
    root: &[u8]
) -> Result<()>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let mut smt_helper = SMTHelper::new(storage.clone());
    let root_key = format!("smt:root:{}", height).into_bytes();
    
    smt_helper.storage.put(&root_key, root)
        .map_err(|e| anyhow::anyhow!("Failed to store state root: {}", e))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::create_test_adapter;

    #[tokio::test]
    async fn test_state_root_management() -> Result<()> {
        let adapter = create_test_adapter();
        let manager = GenericStateRootManager::new(adapter);

        let test_root = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
                            17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];

        // Store state root
        manager.store_state_root(100, &test_root).await?;

        // Retrieve state root
        let retrieved = manager.get_state_root(100).await?;
        assert_eq!(retrieved, Some(test_root.clone()));

        // Non-existent height should return None
        let non_existent = manager.get_state_root(999).await?;
        assert_eq!(non_existent, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_utility_functions() -> Result<()> {
        let adapter = create_test_adapter();
        
        let test_root = vec![1; 32];

        // Store using utility function
        store_state_root_at_height(&adapter, 50, &test_root).await?;

        // Retrieve using utility function
        let retrieved = get_state_root_at_height(&adapter, 50).await?;
        assert_eq!(retrieved, Some(test_root));

        // Non-existent height
        let non_existent = get_state_root_at_height(&adapter, 999).await?;
        assert_eq!(non_existent, None);

        Ok(())
    }
}