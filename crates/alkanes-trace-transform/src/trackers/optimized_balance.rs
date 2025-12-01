use crate::extractor::TraceExtractor;
use crate::types::{AlkaneId, TraceEvent, Result};
use crate::trackers::balance::{ValueTransferExtractor, BalanceChange};
use sqlx::PgPool;
use std::collections::HashMap;

/// Optimized balance tracker that writes directly to proper indexed tables
/// instead of using the generic key-value backend
pub struct OptimizedBalanceTracker {
    pool: PgPool,
}

impl OptimizedBalanceTracker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Process balance changes and update all tables
    pub async fn process_balance_changes(&self, changes: Vec<BalanceChange>) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        
        let mut tx = self.pool.begin().await?;
        
        // Group changes by (address, alkane_id) for aggregation
        let mut aggregates: HashMap<(String, AlkaneId), u128> = HashMap::new();
        
        for change in &changes {
            // Insert/update UTXO-level balance
            self.upsert_utxo_balance(&mut tx, change).await?;
            
            // Accumulate for aggregate update
            let key = (change.address.clone(), change.alkane_id.clone());
            *aggregates.entry(key).or_insert(0) += change.amount;
        }
        
        // Update aggregate balances
        for ((address, alkane_id), amount_delta) in aggregates {
            self.update_aggregate_balance(&mut tx, &address, &alkane_id, amount_delta).await?;
            self.update_holder(&mut tx, &address, &alkane_id, amount_delta).await?;
        }
        
        tx.commit().await?;
        
        Ok(())
    }
    
    /// Insert or update UTXO balance
    async fn upsert_utxo_balance(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        change: &BalanceChange,
    ) -> Result<()> {
        let parts: Vec<&str> = change.outpoint.split(':').collect();
        let txid = parts.get(0).unwrap_or(&"");
        let vout: i32 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        
        sqlx::query(
            r#"INSERT INTO "TraceBalanceUtxo"
               (outpoint_txid, outpoint_vout, address, alkane_block, alkane_tx, amount, block_height, spent)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (outpoint_txid, outpoint_vout, alkane_block, alkane_tx)
               DO UPDATE SET
                   amount = EXCLUDED.amount,
                   spent = EXCLUDED.spent"#
        )
        .bind(txid)
        .bind(vout)
        .bind(&change.address)
        .bind(change.alkane_id.block)
        .bind(change.alkane_id.tx)
        .bind(change.amount.to_string())
        .bind(change.block_height)
        .bind(false) // not spent when created
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Update aggregate balance
    async fn update_aggregate_balance(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        alkane_id: &AlkaneId,
        amount_delta: u128,
    ) -> Result<()> {
        // Get current balance
        let current: Option<String> = sqlx::query_scalar(
            r#"SELECT total_amount::TEXT FROM "TraceBalanceAggregate"
               WHERE address = $1 AND alkane_block = $2 AND alkane_tx = $3"#
        )
        .bind(address)
        .bind(alkane_id.block)
        .bind(alkane_id.tx)
        .fetch_optional(&mut **tx)
        .await?;
        
        let new_total = current
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(0) + amount_delta;
        
        sqlx::query(
            r#"INSERT INTO "TraceBalanceAggregate"
               (address, alkane_block, alkane_tx, total_amount, updated_at)
               VALUES ($1, $2, $3, $4, NOW())
               ON CONFLICT (address, alkane_block, alkane_tx)
               DO UPDATE SET
                   total_amount = EXCLUDED.total_amount,
                   updated_at = NOW()"#
        )
        .bind(address)
        .bind(alkane_id.block)
        .bind(alkane_id.tx)
        .bind(new_total.to_string())
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Update holder enumeration
    async fn update_holder(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        alkane_id: &AlkaneId,
        amount_delta: u128,
    ) -> Result<()> {
        // Get current holder amount
        let current: Option<String> = sqlx::query_scalar(
            r#"SELECT total_amount::TEXT FROM "TraceHolder"
               WHERE alkane_block = $1 AND alkane_tx = $2 AND address = $3"#
        )
        .bind(alkane_id.block)
        .bind(alkane_id.tx)
        .bind(address)
        .fetch_optional(&mut **tx)
        .await?;
        
        let was_zero = current.is_none();
        let new_total = current
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(0) + amount_delta;
        
        sqlx::query(
            r#"INSERT INTO "TraceHolder"
               (alkane_block, alkane_tx, address, total_amount, updated_at)
               VALUES ($1, $2, $3, $4, NOW())
               ON CONFLICT (alkane_block, alkane_tx, address)
               DO UPDATE SET
                   total_amount = EXCLUDED.total_amount,
                   updated_at = NOW()"#
        )
        .bind(alkane_id.block)
        .bind(alkane_id.tx)
        .bind(address)
        .bind(new_total.to_string())
        .execute(&mut **tx)
        .await?;
        
        // Update holder count if this is a new holder
        if was_zero && new_total > 0 {
            sqlx::query(
                r#"INSERT INTO "TraceHolderCount"
                   (alkane_block, alkane_tx, count, updated_at)
                   VALUES ($1, $2, 1, NOW())
                   ON CONFLICT (alkane_block, alkane_tx)
                   DO UPDATE SET
                       count = "TraceHolderCount".count + 1,
                       updated_at = NOW()"#
            )
            .bind(alkane_id.block)
            .bind(alkane_id.tx)
            .execute(&mut **tx)
            .await?;
        }
        
        Ok(())
    }
    
    /// Mark UTXOs as spent
    pub async fn mark_utxos_spent(&self, outpoints: Vec<String>, _block_height: i32) -> Result<()> {
        if outpoints.is_empty() {
            return Ok(());
        }
        
        let mut tx = self.pool.begin().await?;
        
        for outpoint in outpoints {
            let parts: Vec<&str> = outpoint.split(':').collect();
            let txid = parts.get(0).unwrap_or(&"");
            let vout: i32 = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            
            sqlx::query(
                r#"UPDATE "TraceBalanceUtxo"
                   SET spent = true
                   WHERE outpoint_txid = $1 AND outpoint_vout = $2"#
            )
            .bind(txid)
            .bind(vout)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        
        Ok(())
    }
}

/// Optimized balance processor that combines extraction and tracking
pub struct OptimizedBalanceProcessor {
    extractor: ValueTransferExtractor,
    tracker: OptimizedBalanceTracker,
}

impl OptimizedBalanceProcessor {
    pub fn new(pool: PgPool) -> Self {
        Self {
            extractor: ValueTransferExtractor::new(),
            tracker: OptimizedBalanceTracker::new(pool),
        }
    }
    
    /// Process a trace event and update balances
    pub async fn process_trace(&mut self, trace: &TraceEvent) -> Result<()> {
        if let Some(changes) = self.extractor.extract(trace)? {
            self.tracker.process_balance_changes(changes).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TransactionContext, VoutInfo};
    use serde_json::json;
    
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_optimized_balance_tracker() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/alkanes_test".to_string());
        
        let pool = PgPool::connect(&database_url).await.unwrap();
        
        // Apply schema
        crate::schema::apply_schema(&pool).await.unwrap();
        
        let tracker = OptimizedBalanceTracker::new(pool.clone());
        
        let changes = vec![
            BalanceChange {
                outpoint: "abc123:0".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 1000,
                block_height: 100,
            },
            BalanceChange {
                outpoint: "abc123:1".to_string(),
                address: "bc1qtest".to_string(),
                alkane_id: AlkaneId::new(4, 10),
                amount: 500,
                block_height: 100,
            },
        ];
        
        tracker.process_balance_changes(changes).await.unwrap();
        
        // Verify aggregate balance
        let balance: String = sqlx::query_scalar(
            r#"SELECT total_amount::TEXT FROM "TraceBalanceAggregate"
               WHERE address = $1 AND alkane_block = $2 AND alkane_tx = $3"#
        )
        .bind("bc1qtest")
        .bind(4)
        .bind(10i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(balance.parse::<u128>().unwrap(), 1500);
        
        // Verify holder count
        let count: i64 = sqlx::query_scalar(
            r#"SELECT count FROM "TraceHolderCount"
               WHERE alkane_block = $1 AND alkane_tx = $2"#
        )
        .bind(4)
        .bind(10i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        
        assert_eq!(count, 1);
        
        // Clean up
        crate::schema::drop_schema(&pool).await.unwrap();
    }
}
