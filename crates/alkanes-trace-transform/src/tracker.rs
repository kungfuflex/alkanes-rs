use crate::backend::StorageBackend;
use crate::types::Result;

/// Tracks and aggregates state from extracted trace data
pub trait StateTracker: Send + Sync {
    type Input;
    
    /// Name of this tracker
    fn name(&self) -> &'static str;
    
    /// Dependencies on other trackers (by name)
    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
    
    /// Update state based on input data
    fn update<B: StorageBackend>(&mut self, backend: &mut B, input: Self::Input) -> Result<()>;
    
    /// Reset state (for testing)
    fn reset<B: StorageBackend>(&mut self, backend: &mut B) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    
    struct TestTracker;
    
    impl StateTracker for TestTracker {
        type Input = String;
        
        fn name(&self) -> &'static str {
            "test_tracker"
        }
        
        fn update<B: StorageBackend>(&mut self, backend: &mut B, input: String) -> Result<()> {
            backend.set("test_table", b"key", input.as_bytes())?;
            Ok(())
        }
        
        fn reset<B: StorageBackend>(&mut self, backend: &mut B) -> Result<()> {
            backend.delete("test_table", b"key")?;
            Ok(())
        }
    }
    
    #[test]
    fn test_tracker() {
        let mut backend = InMemoryBackend::new();
        let mut tracker = TestTracker;
        
        tracker.update(&mut backend, "test_value".to_string()).unwrap();
        
        let value = backend.get("test_table", b"key").unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }
}
