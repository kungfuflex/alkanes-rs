use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::debug;

/// Extract storage changes from trace events (ReturnContext)
pub fn extract_storage_changes(
    tx: &JsonValue,
    trace_events: &[super::protostone::TraceEventItem],
) -> Result<Vec<StorageChange>> {
    let mut changes = Vec::new();
    
    let txid = tx.get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing txid"))?
        .to_string();
    
    for event in trace_events {
        if event.event_type == "return" {
            if let Some(inner) = event.data.get("inner") {
                if let Some(storage) = inner.get("storage") {
                    if let Some(entries) = storage.as_object() {
                        for (key, value) in entries {
                            changes.push(StorageChange {
                                txid: txid.clone(),
                                vout: event.vout,
                                alkane_id_block: event.alkane_address_block.parse().unwrap_or(0),
                                alkane_id_tx: event.alkane_address_tx.parse().unwrap_or(0),
                                key: key.clone(),
                                value: value.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(changes)
}

#[derive(Debug, Clone)]
pub struct StorageChange {
    pub txid: String,
    pub vout: i32,
    pub alkane_id_block: i32,
    pub alkane_id_tx: i64,
    pub key: String,
    pub value: String,
}

/// Upsert storage changes into database
pub async fn upsert_storage_changes(
    pool: &PgPool,
    block_height: i32,
    changes: &[StorageChange],
) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    
    let mut dbtx = pool.begin().await?;
    
    for change in changes {
        sqlx::query(
            r#"
            insert into "AlkaneStorage"
            ("alkaneIdBlock", "alkaneIdTx", "key", "value", "lastTxid", "lastVout", "blockHeight")
            values ($1, $2, $3, $4, $5, $6, $7)
            on conflict ("alkaneIdBlock", "alkaneIdTx", "key")
            do update set
                "value" = excluded."value",
                "lastTxid" = excluded."lastTxid",
                "lastVout" = excluded."lastVout",
                "blockHeight" = excluded."blockHeight",
                "updatedAt" = now()
            "#
        )
        .bind(change.alkane_id_block)
        .bind(change.alkane_id_tx)
        .bind(&change.key)
        .bind(&change.value)
        .bind(&change.txid)
        .bind(change.vout)
        .bind(block_height)
        .execute(&mut *dbtx)
        .await?;
    }
    
    dbtx.commit().await?;
    debug!("Upserted {} storage changes", changes.len());
    Ok(())
}
