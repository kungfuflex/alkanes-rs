use crate::types::{TraceEvent, Result};

/// Extracts specific data from trace events
pub trait TraceExtractor: Send + Sync {
    type Output;
    
    /// Extract data from a trace event
    fn extract(&self, trace: &TraceEvent) -> Result<Option<Self::Output>>;
    
    /// Name of this extractor for dependency tracking
    fn name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    struct TestExtractor;
    
    impl TraceExtractor for TestExtractor {
        type Output = String;
        
        fn extract(&self, trace: &TraceEvent) -> Result<Option<String>> {
            if trace.event_type == "test_event" {
                Ok(Some("extracted".to_string()))
            } else {
                Ok(None)
            }
        }
        
        fn name(&self) -> &'static str {
            "test_extractor"
        }
    }
    
    #[test]
    fn test_extractor() {
        let extractor = TestExtractor;
        
        let trace = TraceEvent {
            event_type: "test_event".to_string(),
            vout: 0,
            alkane_address_block: "4".to_string(),
            alkane_address_tx: "0".to_string(),
            data: json!({}),
        };
        
        let result = extractor.extract(&trace).unwrap();
        assert_eq!(result, Some("extracted".to_string()));
    }
}
