use anyhow::Result;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use sqlx::Row;

/// Batch upsert records into "AlkaneTransaction" by unique "transactionId".
/// On conflict, updates mutable fields and refreshes "updatedAt".
pub async fn upsert_alkane_transactions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    // (blockHeight, transactionId, transactionIndex, hasTrace, traceSucceed, transactionData)
    items: &[(i32, String, i32, bool, bool, JsonValue)],
) -> Result<()> {
    if items.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 6;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1).min(1500);

    for chunk in items.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"AlkaneTransaction\" (\"blockHeight\", \"transactionId\", \"transactionIndex\", \"hasTrace\", \"traceSucceed\", \"transactionData\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!("(${}, ${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5, base+6));
        }
        q.push_str(
            " on conflict (\"transactionId\") do update set \"blockHeight\" = excluded.\"blockHeight\", \"transactionIndex\" = excluded.\"transactionIndex\", \"hasTrace\" = excluded.\"hasTrace\", \"traceSucceed\" = excluded.\"traceSucceed\", \"transactionData\" = excluded.\"transactionData\", \"updatedAt\" = now()",
        );
        q.push_str(
            " where (\"AlkaneTransaction\".\"blockHeight\", \"AlkaneTransaction\".\"transactionIndex\", \"AlkaneTransaction\".\"hasTrace\", \"AlkaneTransaction\".\"traceSucceed\", \"AlkaneTransaction\".\"transactionData\") is distinct from (excluded.\"blockHeight\", excluded.\"transactionIndex\", excluded.\"hasTrace\", excluded.\"traceSucceed\", excluded.\"transactionData\")",
        );

        let mut qb = sqlx::query(&q);
        for (bh, txid, idx, has_trace, trace_ok, data) in chunk {
            qb = qb
                .bind(bh)
                .bind(txid)
                .bind(idx)
                .bind(has_trace)
                .bind(trace_ok)
                .bind(data);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}

/// Replace TraceEvent rows for a set of txids, then bulk insert provided events.
pub async fn replace_trace_events(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, vout, eventType, data, alkaneAddressBlock, alkaneAddressTx)
    events: &[(String, i32, i32, String, JsonValue, String, String)],
) -> Result<()> {
    if !txids.is_empty() {
        // Use CTE with unnest for better plans at large array sizes
        sqlx::query(
            r#"with ids as (select unnest($1::text[]) as txid)
               delete from "TraceEvent" te using ids
               where te."transactionId" = ids.txid"#,
        )
        .bind(txids)
        .execute(&mut **tx)
        .await?;
    }
    if events.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 7;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1).min(2000);

    for chunk in events.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"TraceEvent\" (\"transactionId\", \"blockHeight\", \"vout\", \"eventType\", \"data\", \"alkaneAddressBlock\", \"alkaneAddressTx\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!("(${}, ${}, ${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5, base+6, base+7));
        }
        let mut qb = sqlx::query(&q);
        for (txid, block_height, vout, etype, data, blk, txnum) in chunk {
            qb = qb.bind(txid).bind(block_height).bind(vout).bind(etype).bind(data).bind(blk).bind(txnum);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}

/// Replace DecodedProtostone rows for a set of txids, then bulk insert provided protostones.
pub async fn replace_decoded_protostones(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, vout, protostoneIndex, blockHeight, decoded_json)
    items: &[(String, i32, i32, i32, JsonValue)],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(
            r#"with ids as (select unnest($1::text[]) as txid)
               delete from "DecodedProtostone" dp using ids
               where dp."transactionId" = ids.txid"#,
        )
        .bind(txids)
        .execute(&mut **tx)
        .await?;
    }
    if items.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 5;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1).min(2000);

    for chunk in items.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"DecodedProtostone\" (\"transactionId\", \"vout\", \"protostoneIndex\", \"blockHeight\", \"decoded\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!("(${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5));
        }
        q.push_str(" on conflict (\"transactionId\", \"vout\", \"protostoneIndex\") do update set \"decoded\" = excluded.\"decoded\", \"updatedAt\" = now() where \"DecodedProtostone\".\"decoded\" is distinct from excluded.\"decoded\"");
        let mut qb = sqlx::query(&q);
        for (txid, vout, idx, block_height, decoded) in chunk {
            qb = qb.bind(txid).bind(vout).bind(idx).bind(block_height).bind(decoded);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}

/// Fetch decoded protostones for given txids, keyed by (transactionId, vout).
/// Returns a map: txid -> (vout -> Vec<(protostoneIndex, decoded_json)>)
pub async fn get_decoded_protostones_by_txid_vout(
    pool: &sqlx::PgPool,
    txids: &[String],
 ) -> Result<HashMap<String, HashMap<i32, Vec<(i32, JsonValue)>>>> {
    let mut out: HashMap<String, HashMap<i32, Vec<(i32, JsonValue)>>> = HashMap::new();
    if txids.is_empty() { return Ok(out); }

    // Query all rows for provided txids (runtime-checked to avoid compile-time DB dependency)
    let rows = sqlx::query(
        r#"select "transactionId" as txid, "vout", "protostoneIndex" as idx, "decoded" from "DecodedProtostone" where "transactionId" = any($1) order by "transactionId", "vout", "protostoneIndex""#
    )
    .bind(txids)
    .fetch_all(pool)
    .await?;

    for r in rows {
        let txid: String = r.try_get("txid")?;
        let vout: i32 = r.try_get("vout")?;
        let idx: i32 = r.try_get("idx")?;
        let decoded: JsonValue = r.try_get("decoded")?;
        out.entry(txid)
            .or_default()
            .entry(vout)
            .or_default()
            .push((idx, decoded));
    }
    Ok(out)
}

/// Replace PoolSwap rows for a set of txids, then bulk insert provided swaps.
pub async fn replace_pool_swaps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, soldTokenBlockId, soldTokenTxId, boughtTokenBlockId, boughtTokenTxId, soldAmount, boughtAmount, sellerAddress, timestamp, successful)
    swaps: &[(String, i32, i32, String, String, String, String, String, String, f64, f64, Option<String>, chrono::DateTime<chrono::Utc>, bool)],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "PoolSwap" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if swaps.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 14;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in swaps.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"PoolSwap\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"poolBlockId\", \"poolTxId\", \"soldTokenBlockId\", \"soldTokenTxId\", \"boughtTokenBlockId\", \"boughtTokenTxId\", \"soldAmount\", \"boughtAmount\", \"sellerAddress\", \"successful\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!("(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5, base+6, base+7, base+8, base+9, base+10, base+11, base+12, base+13, base+14));
        }
        let mut qb = sqlx::query(&q);
        for (txid, bh, idx, pb, pt, sb, st, bb, bt, s_amt, b_amt, seller, ts, success) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(pb)
                .bind(pt)
                .bind(sb)
                .bind(st)
                .bind(bb)
                .bind(bt)
                .bind(s_amt)
                .bind(b_amt)
                .bind(seller)
                .bind(success)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}


/// Replace PoolCreation rows for a set of txids, then bulk insert provided creations.
pub async fn replace_pool_creations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, token0BlockId, token0TxId, token1BlockId, token1TxId, token0Amount, token1Amount, tokenSupply, creatorAddress, timestamp)
    creations: &[(
        String,
        i32,
        i32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    )],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "PoolCreation" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if creations.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 14;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in creations.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"PoolCreation\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"poolBlockId\", \"poolTxId\", \"token0BlockId\", \"token0TxId\", \"token1BlockId\", \"token1TxId\", \"token0Amount\", \"token1Amount\", \"tokenSupply\", \"creatorAddress\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base+1, base+2, base+3, base+4, base+5, base+6, base+7, base+8, base+9, base+10, base+11, base+12, base+13, base+14
            ));
        }
        // Skip duplicate pool creations (same pool can appear in multiple traces)
        q.push_str(" on conflict (\"poolBlockId\", \"poolTxId\") do nothing");
        let mut qb = sqlx::query(&q);
        for (
            txid, bh, idx, pb, pt, t0b, t0t, t1b, t1t, a0, a1, supply, creator, ts
        ) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(pb)
                .bind(pt)
                .bind(t0b)
                .bind(t0t)
                .bind(t1b)
                .bind(t1t)
                .bind(a0)
                .bind(a1)
                .bind(supply)
                .bind(creator)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}


/// Replace PoolMint rows for a set of txids, then bulk insert provided mints.
pub async fn replace_pool_mints(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, lpTokenAmount, token0BlockId, token0TxId, token1BlockId, token1TxId, token0Amount, token1Amount, minterAddress, timestamp, successful)
    mints: &[(
        String,
        i32,
        i32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
        bool,
    )],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "PoolMint" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if mints.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 15;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in mints.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"PoolMint\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"poolBlockId\", \"poolTxId\", \"lpTokenAmount\", \"token0BlockId\", \"token0TxId\", \"token1BlockId\", \"token1TxId\", \"token0Amount\", \"token1Amount\", \"minterAddress\", \"successful\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base+1, base+2, base+3, base+4, base+5, base+6, base+7, base+8, base+9, base+10, base+11, base+12, base+13, base+14, base+15
            ));
        }
        let mut qb = sqlx::query(&q);
        for (
            txid, bh, idx, pb, pt, lp_amt, t0b, t0t, t1b, t1t, a0, a1, minter, ts, success
        ) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(pb)
                .bind(pt)
                .bind(lp_amt)
                .bind(t0b)
                .bind(t0t)
                .bind(t1b)
                .bind(t1t)
                .bind(a0)
                .bind(a1)
                .bind(minter)
                .bind(success)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}


/// Replace PoolBurn rows for a set of txids, then bulk insert provided burns.
pub async fn replace_pool_burns(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, lpTokenAmount, token0BlockId, token0TxId, token1BlockId, token1TxId, token0Amount, token1Amount, burnerAddress, timestamp, successful)
    burns: &[(
        String,
        i32,
        i32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
        bool,
    )],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "PoolBurn" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if burns.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 15;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in burns.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"PoolBurn\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"poolBlockId\", \"poolTxId\", \"lpTokenAmount\", \"token0BlockId\", \"token0TxId\", \"token1BlockId\", \"token1TxId\", \"token0Amount\", \"token1Amount\", \"burnerAddress\", \"successful\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base+1, base+2, base+3, base+4, base+5, base+6, base+7, base+8, base+9, base+10, base+11, base+12, base+13, base+14, base+15
            ));
        }
        let mut qb = sqlx::query(&q);
        for (
            txid, bh, idx, pb, pt, lp_amt, t0b, t0t, t1b, t1t, a0, a1, burner, ts, success
        ) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(pb)
                .bind(pt)
                .bind(lp_amt)
                .bind(t0b)
                .bind(t0t)
                .bind(t1b)
                .bind(t1t)
                .bind(a0)
                .bind(a1)
                .bind(burner)
                .bind(success)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}

/// Replace SubfrostWrap rows for a set of txids, then bulk insert provided wraps.
pub async fn replace_subfrost_wraps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, address, amount, successful, timestamp)
    wraps: &[(
        String,
        i32,
        i32,
        Option<String>,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
    )],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "SubfrostWrap" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if wraps.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 7;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in wraps.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"SubfrostWrap\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"address\", \"amount\", \"successful\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base+1, base+2, base+3, base+4, base+5, base+6, base+7
            ));
        }
        let mut qb = sqlx::query(&q);
        for (txid, bh, idx, addr, amt, success, ts) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(addr)
                .bind(amt)
                .bind(success)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}

/// Replace SubfrostUnwrap rows for a set of txids, then bulk insert provided unwraps.
pub async fn replace_subfrost_unwraps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    txids: &[String],
    // (transactionId, blockHeight, transactionIndex, address, amount, successful, timestamp)
    unwraps: &[(
        String,
        i32,
        i32,
        Option<String>,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
    )],
) -> Result<()> {
    if !txids.is_empty() {
        sqlx::query(r#"delete from "SubfrostUnwrap" where "transactionId" = any($1)"#)
            .bind(txids)
            .execute(&mut **tx)
            .await?;
    }
    if unwraps.is_empty() { return Ok(()); }

    const MAX_PARAMS: usize = 65535;
    const PER_ROW: usize = 7;
    let max_rows = (MAX_PARAMS / PER_ROW).saturating_sub(8).max(1);

    for chunk in unwraps.chunks(max_rows) {
        let mut q = String::from(
            "insert into \"SubfrostUnwrap\" (\"transactionId\", \"blockHeight\", \"transactionIndex\", \"address\", \"amount\", \"successful\", \"timestamp\") values ",
        );
        for i in 0..chunk.len() {
            if i > 0 { q.push(','); }
            let base = i * PER_ROW;
            q.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                base+1, base+2, base+3, base+4, base+5, base+6, base+7
            ));
        }
        let mut qb = sqlx::query(&q);
        for (txid, bh, idx, addr, amt, success, ts) in chunk {
            qb = qb
                .bind(txid)
                .bind(bh)
                .bind(idx)
                .bind(addr)
                .bind(amt)
                .bind(success)
                .bind(ts);
        }
        qb.execute(&mut **tx).await?;
    }
    Ok(())
}
