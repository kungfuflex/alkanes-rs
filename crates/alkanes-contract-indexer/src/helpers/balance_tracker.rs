use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub alkane_id_block: i32,
    pub alkane_id_tx: i64,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct OutpointBalance {
    pub outpoint_txid: String,
    pub outpoint_vout: i32,
    pub address: String,
    pub changes: Vec<BalanceChange>,
}

/// Extract balance changes from a transaction's trace events
pub fn extract_balance_changes(
    tx: &JsonValue,
    trace_events: &[super::protostone::TraceEventItem],
) -> Result<Vec<OutpointBalance>> {
    let mut outpoint_balances: HashMap<(String, i32), OutpointBalance> = HashMap::new();
    
    // Get transaction outputs for address resolution
    let outputs = tx.get("vout")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing vout"))?;
    
    let txid = tx.get("txid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing txid"))?
        .to_string();
    
    // Process each trace event
    for event in trace_events {
        if event.event_type == "value_transfer" {
            let vout = event.vout;
            let data = &event.data;
            let empty_vec = vec![];
            let transfers = data.get("transfers")
                .and_then(|v| v.as_array())
                .unwrap_or(&empty_vec);
            let redirect_to = data.get("redirect_to")
                .and_then(|v| v.as_i64())
                .unwrap_or(vout as i64) as i32;
            
            // Get address for target vout
            if let Some(address) = get_address_from_output(outputs, redirect_to) {
                let key = (txid.clone(), redirect_to);
                let entry = outpoint_balances.entry(key).or_insert_with(|| {
                    OutpointBalance {
                        outpoint_txid: txid.clone(),
                        outpoint_vout: redirect_to,
                        address: address.clone(),
                        changes: Vec::new(),
                    }
                });
                
                for transfer in transfers {
                    let alkane_id = transfer.get("id").or_else(|| transfer.get("alkaneId")).unwrap_or(&JsonValue::Null);
                    let block = alkane_id.get("block")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    let tx_num = alkane_id.get("tx")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let amount = transfer.get("amount")
                        .or_else(|| transfer.get("value"))
                        .and_then(|v| {
                            v.as_str().map(|s| s.to_string())
                                .or_else(|| v.as_u64().map(|n| n.to_string()))
                        })
                        .unwrap_or_else(|| "0".to_string());
                    
                    if block > 0 && tx_num > 0 {
                        entry.changes.push(BalanceChange {
                            alkane_id_block: block,
                            alkane_id_tx: tx_num,
                            amount,
                        });
                    }
                }
            }
        }
    }
    
    Ok(outpoint_balances.into_values().collect())
}

fn get_address_from_output(outputs: &[JsonValue], vout: i32) -> Option<String> {
    outputs.get(vout as usize)
        .and_then(|out| out.get("scriptPubKey"))
        .and_then(|spk| spk.get("address"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
}

/// Upsert UTXO balances into database
pub async fn upsert_utxo_balances(
    pool: &PgPool,
    block_height: i32,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    let mut tx = pool.begin().await?;
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            sqlx::query(
                r#"
                insert into "AlkaneBalanceUtxo" 
                ("address", "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount", "blockHeight")
                values ($1, $2, $3, $4, $5, $6, $7)
                on conflict ("outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx")
                do update set 
                    "amount" = excluded."amount",
                    "updatedAt" = now()
                "#
            )
            .bind(&ob.address)
            .bind(&ob.outpoint_txid)
            .bind(ob.outpoint_vout)
            .bind(change.alkane_id_block)
            .bind(change.alkane_id_tx)
            .bind(&change.amount)
            .bind(block_height)
            .execute(&mut *tx)
            .await?;
        }
    }
    
    tx.commit().await?;
    debug!("Upserted {} UTXO balances", outpoint_balances.len());
    Ok(())
}

/// Update aggregate address balances
pub async fn update_address_balances(
    pool: &PgPool,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    // Group by address and alkane
    let mut aggregates: HashMap<(String, i32, i64), i128> = HashMap::new();
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            let key = (ob.address.clone(), change.alkane_id_block, change.alkane_id_tx);
            let amount: i128 = change.amount.parse().unwrap_or(0);
            *aggregates.entry(key).or_insert(0) += amount;
        }
    }
    
    let mut dbtx = pool.begin().await?;
    let agg_count = aggregates.len();
    
    for ((address, block, tx_num), amount) in aggregates.into_iter() {
        if amount != 0 {
            sqlx::query(
                r#"
                insert into "AlkaneBalance"
                ("address", "alkaneIdBlock", "alkaneIdTx", "amount")
                values ($1, $2, $3, $4)
                on conflict ("address", "alkaneIdBlock", "alkaneIdTx")
                do update set
                    "amount" = (
                        (coalesce("AlkaneBalance"."amount", '0')::numeric + $4::numeric)::text
                    ),
                    "updatedAt" = now()
                "#
            )
            .bind(&address)
            .bind(block)
            .bind(tx_num)
            .bind(amount.to_string())
            .execute(&mut *dbtx)
            .await?;
        }
    }
    
    dbtx.commit().await?;
    debug!("Updated {} address balances", agg_count);
    Ok(())
}

/// Refresh holder materialized data for modified alkanes
pub async fn refresh_holders_for_block(
    pool: &PgPool,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    if outpoint_balances.is_empty() {
        return Ok(());
    }
    
    // Collect unique alkanes that were modified
    let mut alkanes: HashMap<(i32, i64), ()> = HashMap::new();
    for ob in outpoint_balances {
        for change in &ob.changes {
            alkanes.insert((change.alkane_id_block, change.alkane_id_tx), ());
        }
    }
    
    for ((block, tx_num), _) in alkanes {
        refresh_holders(pool, block, tx_num).await?;
    }
    
    Ok(())
}

/// Refresh holder materialized data for an alkane
async fn refresh_holders(
    pool: &PgPool,
    alkane_id_block: i32,
    alkane_id_tx: i64,
) -> Result<()> {
    // Delete old entries
    sqlx::query(
        r#"delete from "AlkaneHolder" where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2"#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Insert fresh data
    sqlx::query(
        r#"
        insert into "AlkaneHolder" ("alkaneIdBlock", "alkaneIdTx", "address", "totalAmount")
        select "alkaneIdBlock", "alkaneIdTx", "address", "amount"
        from "AlkaneBalance"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
          and "amount"::numeric > 0
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Update count
    sqlx::query(
        r#"
        insert into "AlkaneHolderCount" ("alkaneIdBlock", "alkaneIdTx", "count")
        select $1, $2, count(*)
        from "AlkaneHolder"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
        on conflict ("alkaneIdBlock", "alkaneIdTx")
        do update set "count" = excluded."count", "lastUpdated" = now()
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    debug!("Refreshed holders for alkane {}:{}", alkane_id_block, alkane_id_tx);
    Ok(())
}
