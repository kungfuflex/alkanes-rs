use crate::types::{QueryFilter, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Abstract storage backend for trace state
pub trait StorageBackend: Send + Sync {
    /// Get a single value by key
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// Set a single value
    fn set(&mut self, table: &str, key: &[u8], value: &[u8]) -> Result<()>;
    
    /// Delete a key
    fn delete(&mut self, table: &str, key: &[u8]) -> Result<()>;
    
    /// Batch insert multiple records
    fn batch_insert(&mut self, table: &str, records: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()>;
    
    /// Query records with filter
    fn query(&self, table: &str, filter: QueryFilter) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
    
    /// Scan all records in a table
    fn scan(&self, table: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
    
    /// Clear all data (for testing)
    fn clear(&mut self) -> Result<()>;
}

/// In-memory backend for testing
#[derive(Clone)]
pub struct InMemoryBackend {
    data: Arc<RwLock<HashMap<String, HashMap<Vec<u8>, Vec<u8>>>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBackend for InMemoryBackend {
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().unwrap();
        Ok(data
            .get(table)
            .and_then(|t| t.get(key))
            .cloned())
    }
    
    fn set(&mut self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.entry(table.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_vec(), value.to_vec());
        Ok(())
    }
    
    fn delete(&mut self, table: &str, key: &[u8]) -> Result<()> {
        let mut data = self.data.write().unwrap();
        if let Some(table_data) = data.get_mut(table) {
            table_data.remove(key);
        }
        Ok(())
    }
    
    fn batch_insert(&mut self, table: &str, records: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        let mut data = self.data.write().unwrap();
        let table_data = data.entry(table.to_string())
            .or_insert_with(HashMap::new);
        
        for (key, value) in records {
            table_data.insert(key, value);
        }
        Ok(())
    }
    
    fn query(&self, table: &str, filter: QueryFilter) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let data = self.data.read().unwrap();
        let Some(table_data) = data.get(table) else {
            return Ok(vec![]);
        };
        
        let results: Vec<(Vec<u8>, Vec<u8>)> = table_data
            .iter()
            .filter(|(k, _v)| match &filter {
                QueryFilter::Equals(target) => *k == target,
                QueryFilter::In(targets) => targets.contains(k),
                QueryFilter::Range { min, max } => {
                    let matches_min = min.as_ref().map_or(true, |m| k.as_slice() >= m.as_slice());
                    let matches_max = max.as_ref().map_or(true, |m| k.as_slice() <= m.as_slice());
                    matches_min && matches_max
                }
                QueryFilter::Prefix(prefix) => k.starts_with(prefix),
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        Ok(results)
    }
    
    fn scan(&self, table: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let data = self.data.read().unwrap();
        Ok(data
            .get(table)
            .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default())
    }
    
    fn clear(&mut self) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.clear();
        Ok(())
    }
}

#[cfg(feature = "postgres")]
pub mod postgres {
    use super::*;
    use sqlx::PgPool;
    
    /// Postgres backend for production
    pub struct PostgresBackend {
        pool: PgPool,
    }
    
    impl PostgresBackend {
        pub fn new(pool: PgPool) -> Self {
            Self { pool }
        }
        
        /// Ensure table exists
        async fn ensure_table(&self, table: &str) -> Result<()> {
            // Create table if not exists with key-value schema
            let query = format!(
                r#"CREATE TABLE IF NOT EXISTS "{}" (
                    key BYTEA PRIMARY KEY,
                    value BYTEA NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )"#,
                table
            );
            
            sqlx::query(&query)
                .execute(&self.pool)
                .await?;
            
            Ok(())
        }
    }
    
    impl StorageBackend for PostgresBackend {
        fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
            let pool = self.pool.clone();
            let table = table.to_string();
            let key = key.to_vec();
            
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let query = format!(r#"SELECT value FROM "{}" WHERE key = $1"#, table);
                    sqlx::query_scalar::<_, Vec<u8>>(&query)
                        .bind(&key)
                        .fetch_optional(&pool)
                        .await
                })
            })?;
            
            Ok(result)
        }
        
        fn set(&mut self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
            let pool = self.pool.clone();
            let table = table.to_string();
            let key = key.to_vec();
            let value = value.to_vec();
            
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    // Ensure table exists
                    let create_query = format!(
                        r#"CREATE TABLE IF NOT EXISTS "{}" (
                            key BYTEA PRIMARY KEY,
                            value BYTEA NOT NULL,
                            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                        )"#,
                        table
                    );
                    sqlx::query(&create_query).execute(&pool).await?;
                    
                    // Upsert key-value
                    let query = format!(
                        r#"INSERT INTO "{}" (key, value, updated_at) 
                           VALUES ($1, $2, NOW())
                           ON CONFLICT (key) DO UPDATE 
                           SET value = EXCLUDED.value, updated_at = NOW()"#,
                        table
                    );
                    sqlx::query(&query)
                        .bind(&key)
                        .bind(&value)
                        .execute(&pool)
                        .await?;
                    
                    Ok::<(), anyhow::Error>(())
                })
            })?;
            
            Ok(())
        }
        
        fn delete(&mut self, table: &str, key: &[u8]) -> Result<()> {
            let pool = self.pool.clone();
            let table = table.to_string();
            let key = key.to_vec();
            
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let query = format!(r#"DELETE FROM "{}" WHERE key = $1"#, table);
                    sqlx::query(&query)
                        .bind(&key)
                        .execute(&pool)
                        .await?;
                    
                    Ok::<(), anyhow::Error>(())
                })
            })?;
            
            Ok(())
        }
        
        fn batch_insert(&mut self, table: &str, records: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
            if records.is_empty() {
                return Ok(());
            }
            
            let pool = self.pool.clone();
            let table = table.to_string();
            
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    // Ensure table exists
                    let create_query = format!(
                        r#"CREATE TABLE IF NOT EXISTS "{}" (
                            key BYTEA PRIMARY KEY,
                            value BYTEA NOT NULL,
                            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                        )"#,
                        table
                    );
                    sqlx::query(&create_query).execute(&pool).await?;
                    
                    // Build batch upsert
                    let mut query_builder = sqlx::QueryBuilder::new(
                        format!(r#"INSERT INTO "{}" (key, value, updated_at) "#, table)
                    );
                    
                    query_builder.push_values(records.iter(), |mut b, (key, value)| {
                        b.push_bind(key)
                            .push_bind(value)
                            .push("NOW()");
                    });
                    
                    query_builder.push(
                        " ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"
                    );
                    
                    query_builder.build().execute(&pool).await?;
                    
                    Ok::<(), anyhow::Error>(())
                })
            })?;
            
            Ok(())
        }
        
        fn query(&self, table: &str, filter: QueryFilter) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
            let pool = self.pool.clone();
            let table = table.to_string();
            
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let query = match filter {
                        QueryFilter::Equals(ref target) => {
                            let q = format!(r#"SELECT key, value FROM "{}" WHERE key = $1"#, table);
                            sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(&q)
                                .bind(target)
                                .fetch_all(&pool)
                                .await
                        }
                        QueryFilter::Prefix(ref prefix) => {
                            let q = format!(
                                r#"SELECT key, value FROM "{}" WHERE key >= $1 AND key < $2"#,
                                table
                            );
                            let mut upper_bound = prefix.clone();
                            if let Some(last) = upper_bound.last_mut() {
                                *last = last.saturating_add(1);
                            }
                            sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(&q)
                                .bind(prefix)
                                .bind(&upper_bound)
                                .fetch_all(&pool)
                                .await
                        }
                        QueryFilter::Range { ref min, ref max } => {
                            let q = if min.is_some() && max.is_some() {
                                format!(
                                    r#"SELECT key, value FROM "{}" WHERE key >= $1 AND key <= $2"#,
                                    table
                                )
                            } else if min.is_some() {
                                format!(r#"SELECT key, value FROM "{}" WHERE key >= $1"#, table)
                            } else if max.is_some() {
                                format!(r#"SELECT key, value FROM "{}" WHERE key <= $1"#, table)
                            } else {
                                format!(r#"SELECT key, value FROM "{}""#, table)
                            };
                            
                            let mut query = sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(&q);
                            if let Some(m) = min {
                                query = query.bind(m);
                            }
                            if let Some(m) = max {
                                query = query.bind(m);
                            }
                            query.fetch_all(&pool).await
                        }
                        QueryFilter::In(ref targets) => {
                            // For IN queries, we'll do individual lookups for simplicity
                            let mut results = Vec::new();
                            for target in targets {
                                let q = format!(r#"SELECT key, value FROM "{}" WHERE key = $1"#, table);
                                if let Ok(Some(row)) = sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(&q)
                                    .bind(target)
                                    .fetch_optional(&pool)
                                    .await
                                {
                                    results.push(row);
                                }
                            }
                            Ok(results)
                        }
                    }?;
                    
                    Ok::<Vec<(Vec<u8>, Vec<u8>)>, anyhow::Error>(query)
                })
            })?;
            
            Ok(result)
        }
        
        fn scan(&self, table: &str) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
            let pool = self.pool.clone();
            let table = table.to_string();
            
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let query = format!(r#"SELECT key, value FROM "{}""#, table);
                    sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(&query)
                        .fetch_all(&pool)
                        .await
                })
            })?;
            
            Ok(result)
        }
        
        fn clear(&mut self) -> Result<()> {
            // For safety, we don't implement a global clear in production
            // This should only be used in tests
            Err(anyhow::anyhow!("Clear not supported in PostgresBackend"))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_in_memory_backend() {
        let mut backend = InMemoryBackend::new();
        
        // Test set/get
        backend.set("test", b"key1", b"value1").unwrap();
        assert_eq!(backend.get("test", b"key1").unwrap(), Some(b"value1".to_vec()));
        
        // Test batch insert
        backend.batch_insert("test", vec![
            (b"key2".to_vec(), b"value2".to_vec()),
            (b"key3".to_vec(), b"value3".to_vec()),
        ]).unwrap();
        
        // Test scan
        let all = backend.scan("test").unwrap();
        assert_eq!(all.len(), 3);
        
        // Test query with prefix filter
        let filtered = backend.query("test", QueryFilter::Prefix(b"key".to_vec())).unwrap();
        assert_eq!(filtered.len(), 3);
        
        // Test delete
        backend.delete("test", b"key1").unwrap();
        assert_eq!(backend.get("test", b"key1").unwrap(), None);
    }
}
