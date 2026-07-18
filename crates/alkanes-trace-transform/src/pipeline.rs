use crate::backend::StorageBackend;
use crate::extractor::TraceExtractor;
use crate::tracker::StateTracker;
use crate::types::{TraceEvent, TransactionContext, Result};
use std::collections::HashMap;

/// Orchestrates extractors and trackers with dependency resolution
pub struct TransformPipeline {
    extractors: HashMap<String, Box<dyn ExtractorWrapper>>,
    trackers: HashMap<String, Box<dyn TrackerWrapper>>,
    execution_order: Vec<String>,
}

// Type-erased wrappers for dynamic dispatch
trait ExtractorWrapper: Send + Sync {
    fn extract(&self, trace: &TraceEvent) -> Result<Option<Box<dyn std::any::Any>>>;
    fn name(&self) -> &'static str;
}

trait TrackerWrapper: Send + Sync {
    fn update_boxed(&mut self, backend: &mut dyn std::any::Any, input: Box<dyn std::any::Any>) -> Result<()>;
    fn reset_boxed(&mut self, backend: &mut dyn std::any::Any) -> Result<()>;
    fn name(&self) -> &'static str;
    fn dependencies(&self) -> Vec<&'static str>;
}

// Concrete wrapper implementations
struct ExtractorWrapperImpl<E: TraceExtractor + 'static> {
    extractor: E,
}

impl<E: TraceExtractor + 'static> ExtractorWrapper for ExtractorWrapperImpl<E> {
    fn extract(&self, trace: &TraceEvent) -> Result<Option<Box<dyn std::any::Any>>> {
        Ok(self.extractor.extract(trace)?.map(|output| Box::new(output) as Box<dyn std::any::Any>))
    }
    
    fn name(&self) -> &'static str {
        self.extractor.name()
    }
}

struct TrackerWrapperImpl<T: StateTracker + 'static, B: StorageBackend + 'static> {
    tracker: T,
    _phantom: std::marker::PhantomData<B>,
}

impl<T: StateTracker + 'static, B: StorageBackend + 'static> TrackerWrapper for TrackerWrapperImpl<T, B> {
    fn update_boxed(&mut self, backend: &mut dyn std::any::Any, input: Box<dyn std::any::Any>) -> Result<()> {
        let backend = backend.downcast_mut::<B>()
            .ok_or_else(|| anyhow::anyhow!("Backend type mismatch"))?;
        let typed_input = input.downcast::<T::Input>()
            .map_err(|_| anyhow::anyhow!("Type mismatch in tracker input"))?;
        self.tracker.update(backend, *typed_input)
    }
    
    fn reset_boxed(&mut self, backend: &mut dyn std::any::Any) -> Result<()> {
        let backend = backend.downcast_mut::<B>()
            .ok_or_else(|| anyhow::anyhow!("Backend type mismatch"))?;
        self.tracker.reset(backend)
    }
    
    fn name(&self) -> &'static str {
        self.tracker.name()
    }
    
    fn dependencies(&self) -> Vec<&'static str> {
        self.tracker.dependencies()
    }
}

impl TransformPipeline {
    pub fn new() -> Self {
        Self {
            extractors: HashMap::new(),
            trackers: HashMap::new(),
            execution_order: Vec::new(),
        }
    }
    
    /// Add an extractor to the pipeline
    pub fn add_extractor<E: TraceExtractor + 'static>(&mut self, extractor: E) {
        let name = extractor.name().to_string();
        self.extractors.insert(
            name,
            Box::new(ExtractorWrapperImpl { extractor }),
        );
    }
    
    /// Add a tracker to the pipeline
    pub fn add_tracker<T: StateTracker + 'static, B: StorageBackend + 'static>(&mut self, tracker: T) {
        let name = tracker.name().to_string();
        self.trackers.insert(
            name.clone(),
            Box::new(TrackerWrapperImpl::<T, B> { 
                tracker,
                _phantom: std::marker::PhantomData,
            }),
        );
        self.compute_execution_order();
    }
    
    /// Compute execution order based on dependencies (topological sort)
    fn compute_execution_order(&mut self) {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();
        
        fn visit(
            name: &str,
            trackers: &HashMap<String, Box<dyn TrackerWrapper>>,
            visited: &mut std::collections::HashSet<String>,
            visiting: &mut std::collections::HashSet<String>,
            order: &mut Vec<String>,
        ) {
            if visited.contains(name) {
                return;
            }
            
            if visiting.contains(name) {
                panic!("Circular dependency detected: {}", name);
            }
            
            visiting.insert(name.to_string());
            
            if let Some(tracker) = trackers.get(name) {
                for dep in tracker.dependencies() {
                    visit(dep, trackers, visited, visiting, order);
                }
            }
            
            visiting.remove(name);
            visited.insert(name.to_string());
            order.push(name.to_string());
        }
        
        for name in self.trackers.keys() {
            visit(name, &self.trackers, &mut visited, &mut visiting, &mut order);
        }
        
        self.execution_order = order;
    }
    
    /// Process a single trace event
    pub fn process_trace<B: StorageBackend + 'static>(
        &mut self,
        backend: &mut B,
        trace: &TraceEvent,
    ) -> Result<()> {
        // Extract data from all extractors
        let mut extracted_data: HashMap<String, Box<dyn std::any::Any>> = HashMap::new();
        
        for (name, extractor) in &self.extractors {
            if let Some(output) = extractor.extract(trace)? {
                extracted_data.insert(name.clone(), output);
            }
        }
        
        // Update trackers in dependency order
        let backend_any: &mut dyn std::any::Any = backend;
        for tracker_name in &self.execution_order {
            if let Some(tracker) = self.trackers.get_mut(tracker_name) {
                // Find matching extractor output
                if let Some(input) = extracted_data.remove(tracker_name) {
                    tracker.update_boxed(backend_any, input)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Process all traces from a transaction
    pub fn process_transaction<B: StorageBackend + 'static>(
        &mut self,
        backend: &mut B,
        _context: &TransactionContext,
        traces: Vec<TraceEvent>,
    ) -> Result<()> {
        for trace in traces {
            self.process_trace(backend, &trace)?;
        }
        Ok(())
    }
    
    /// Process all traces from a block
    pub fn process_block<B: StorageBackend + 'static>(
        &mut self,
        backend: &mut B,
        transactions: Vec<(TransactionContext, Vec<TraceEvent>)>,
    ) -> Result<()> {
        for (context, traces) in transactions {
            self.process_transaction(backend, &context, traces)?;
        }
        Ok(())
    }
    
    /// Reset all trackers (for testing)
    pub fn reset<B: StorageBackend + 'static>(&mut self, backend: &mut B) -> Result<()> {
        let backend_any: &mut dyn std::any::Any = backend;
        for tracker in self.trackers.values_mut() {
            tracker.reset_boxed(backend_any)?;
        }
        Ok(())
    }
}

impl Default for TransformPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use serde_json::json;
    
    struct TestExtractor;
    impl TraceExtractor for TestExtractor {
        type Output = String;
        fn extract(&self, trace: &TraceEvent) -> Result<Option<String>> {
            if trace.event_type == "test" {
                Ok(Some("data".to_string()))
            } else {
                Ok(None)
            }
        }
        fn name(&self) -> &'static str { "test_extractor" }
    }
    
    struct TestTracker;
    impl StateTracker for TestTracker {
        type Input = String;
        fn name(&self) -> &'static str { "test_extractor" }
        fn update<B: StorageBackend>(&mut self, backend: &mut B, input: String) -> Result<()> {
            backend.set("test", b"key", input.as_bytes())
        }
        fn reset<B: StorageBackend>(&mut self, backend: &mut B) -> Result<()> {
            backend.delete("test", b"key")
        }
    }
    
    #[test]
    fn test_pipeline() {
        let mut backend = InMemoryBackend::new();
        let mut pipeline = TransformPipeline::new();
        
        pipeline.add_extractor(TestExtractor);
        pipeline.add_tracker::<TestTracker, InMemoryBackend>(TestTracker);
        
        let trace = TraceEvent {
            event_type: "test".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({}),
        };
        
        pipeline.process_trace(&mut backend, &trace).unwrap();
        
        let value = backend.get("test", b"key").unwrap();
        assert_eq!(value, Some(b"data".to_vec()));
    }
}
