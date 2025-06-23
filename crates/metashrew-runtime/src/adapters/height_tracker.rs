//! Generic height tracking implementation

use anyhow::Result;
use async_trait::async_trait;
use crate::{to_labeled_key, KeyValueStoreLike, TIP_HEIGHT_KEY};
use super::traits::HeightTracker;

/// Generic height tracker that works with any KeyValueStoreLike implementation
pub struct GenericHeightTracker<T: KeyValueStoreLike> {
    storage: T,
}

impl<T: KeyValueStoreLike> GenericHeightTracker<T> {
    pub fn new(storage: T) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl<T: KeyValueStoreLike + Send + Sync> HeightTracker for GenericHeightTracker<T>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    async fn get_current_height(&self) -> Result<u32> {
        // Try tip height first (used by main runtime)
        let tip_key = "/__INTERNAL/tip-height".as_bytes().to_vec();
        if let Ok(Some(height_bytes)) = self.storage.get_immutable(&tip_key) {
            if height_bytes.len() >= 4 {
                let height = u32::from_le_bytes([
                    height_bytes[0],
                    height_bytes[1],
                    height_bytes[2],
                    height_bytes[3],
                ]);
                return Ok(height);
            }
        }

        // Fall back to indexed height
        self.get_indexed_height().await
    }

    async fn set_current_height(&mut self, height: u32) -> Result<()> {
        let tip_key = "/__INTERNAL/tip-height".as_bytes().to_vec();
        let height_bytes = height.to_le_bytes().to_vec();
        self.storage.put(&tip_key, &height_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to set current height: {}", e))?;
        Ok(())
    }

    async fn get_indexed_height(&self) -> Result<u32> {
        let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        let labeled_key = to_labeled_key(&height_key);
        
        match self.storage.get_immutable(&labeled_key) {
            Ok(Some(bytes)) if bytes.len() >= 4 => {
                let height_bytes: [u8; 4] = bytes[..4].try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid height data"))?;
                Ok(u32::from_le_bytes(height_bytes))
            }
            Ok(_) => Ok(0), // No height found, start from 0
            Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
        }
    }

    async fn set_indexed_height(&mut self, height: u32) -> Result<()> {
        let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
        let labeled_key = to_labeled_key(&height_key);
        let height_bytes = height.to_le_bytes().to_vec();
        
        self.storage.put(&labeled_key, &height_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to set indexed height: {}", e))?;
        Ok(())
    }
}

/// Utility function to query height from any storage backend
pub async fn query_height<T: KeyValueStoreLike>(storage: &T, start_block: u32) -> Result<u32>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let height_key = TIP_HEIGHT_KEY.as_bytes().to_vec();
    let labeled_key = to_labeled_key(&height_key);
    
    match storage.get_immutable(&labeled_key) {
        Ok(Some(bytes)) if !bytes.is_empty() => {
            if bytes.len() >= 4 {
                let height_bytes: [u8; 4] = bytes[..4].try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid height data"))?;
                Ok(u32::from_le_bytes(height_bytes))
            } else {
                Ok(start_block)
            }
        }
        Ok(_) => Ok(start_block), // No height found
        Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::create_test_adapter;

    #[tokio::test]
    async fn test_height_tracking() -> Result<()> {
        let mut adapter = create_test_adapter();
        let tracker = GenericHeightTracker::new(adapter.clone());

        // Initial height should be 0
        assert_eq!(tracker.get_indexed_height().await?, 0);

        // Set height and verify
        tracker.set_indexed_height(42).await?;
        assert_eq!(tracker.get_indexed_height().await?, 42);

        // Test current height
        tracker.set_current_height(100).await?;
        assert_eq!(tracker.get_current_height().await?, 100);

        Ok(())
    }

    #[tokio::test]
    async fn test_query_height_utility() -> Result<()> {
        let mut adapter = create_test_adapter();
        
        // Should return start_block when no height is set
        assert_eq!(query_height(&adapter, 10).await?, 10);

        // Set a height and verify
        let tracker = GenericHeightTracker::new(adapter.clone());
        tracker.set_indexed_height(50).await?;
        assert_eq!(query_height(&adapter, 10).await?, 50);

        Ok(())
    }
}