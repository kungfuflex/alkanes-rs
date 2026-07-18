use crate::backend::StorageBackend;
use crate::types::Result;
use crate::types::QueryParams;

/// Service for querying transformed/aggregated data
pub trait QueryService: Send + Sync {
    type Output;
    
    /// Query data from backend
    fn query<B: StorageBackend>(&self, backend: &B, params: QueryParams) -> Result<Self::Output>;
    
    /// Name of this query service
    fn name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    
    struct TestQueryService;
    
    impl QueryService for TestQueryService {
        type Output = Option<String>;
        
        fn query<B: StorageBackend>(&self, backend: &B, _params: QueryParams) -> Result<Option<String>> {
            let value = backend.get("test_table", b"key")?;
            Ok(value.map(|v| String::from_utf8(v).unwrap()))
        }
        
        fn name(&self) -> &'static str {
            "test_query"
        }
    }
    
    #[test]
    fn test_query_service() {
        let mut backend = InMemoryBackend::new();
        backend.set("test_table", b"key", b"test_value").unwrap();
        
        let service = TestQueryService;
        let result = service.query(&backend, QueryParams::default()).unwrap();
        
        assert_eq!(result, Some("test_value".to_string()));
    }
}
