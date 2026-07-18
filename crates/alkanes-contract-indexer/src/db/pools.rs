use anyhow::Result;
use sqlx::Row;

pub async fn get_existing_pools_for_factory(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    factory_block_id: &str,
    factory_tx_id: &str,
) -> Result<std::collections::HashMap<(String, String), String>> {
    let rows = sqlx::query(
        r#"select id, "poolBlockId", "poolTxId" from "Pool"
           where "factoryBlockId" = $1 and "factoryTxId" = $2"#
    )
    .bind(factory_block_id)
    .bind(factory_tx_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut map = std::collections::HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let pb: String = row.get("poolBlockId");
        let pt: String = row.get("poolTxId");
        map.insert((pb, pt), id);
    }
    Ok(map)
}

pub async fn insert_new_pools(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    factory_block_id: &str,
    factory_tx_id: &str,
    // (pool_block, pool_tx, token0_block, token0_tx, token1_block, token1_tx, pool_name)
    new_pools: &[(String, String, String, String, String, String, String)],
) -> Result<()> {
    if new_pools.is_empty() { return Ok(()); }

    // new_pools tuple: (pool_block, pool_tx, token0_block, token0_tx, token1_block, token1_tx, pool_name)
    let mut q = String::from("insert into \"Pool\" (\"factoryBlockId\", \"factoryTxId\", \"poolBlockId\", \"poolTxId\", \"token0BlockId\", \"token0TxId\", \"token1BlockId\", \"token1TxId\", \"poolName\") values ");
    for i in 0..new_pools.len() {
        if i > 0 { q.push_str(","); }
        let base = i * 9;
        q.push_str(&format!("(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5, base+6, base+7, base+8, base+9));
    }
    q.push_str(" on conflict (\"poolBlockId\", \"poolTxId\") do nothing");
    let mut qb = sqlx::query(&q);
    for (pool_block, pool_tx, token0_block, token0_tx, token1_block, token1_tx, pool_name) in new_pools.iter().map(|t| (t.0.as_str(), t.1.as_str(), t.2.as_str(), t.3.as_str(), t.4.as_str(), t.5.as_str(), t.6.as_str())) {
        qb = qb
            .bind(factory_block_id)
            .bind(factory_tx_id)
            .bind(pool_block)
            .bind(pool_tx)
            .bind(token0_block)
            .bind(token0_tx)
            .bind(token1_block)
            .bind(token1_tx)
            .bind(pool_name);
    }
    qb.execute(&mut **tx).await?;
    Ok(())
}

pub async fn get_pool_ids_for_pairs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    factory_block_id: &str,
    factory_tx_id: &str,
    pairs: &[(String, String)],
) -> Result<std::collections::HashMap<(String, String), String>> {
    if pairs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let block_ids: Vec<&str> = pairs.iter().map(|(pb, _)| pb.as_str()).collect();
    let tx_ids: Vec<&str> = pairs.iter().map(|(_, pt)| pt.as_str()).collect();
    let rows = sqlx::query(
        r#"select id, "poolBlockId", "poolTxId" from "Pool"
           where ("factoryBlockId" = $1 and "factoryTxId" = $2) and ("poolBlockId", "poolTxId") in (
                select pb, pt from unnest($3::text[], $4::text[]) as t(pb, pt)
           )"#
    )
    .bind(factory_block_id)
    .bind(factory_tx_id)
    .bind(block_ids.as_slice())
    .bind(tx_ids.as_slice())
    .fetch_all(&mut **tx)
    .await?;

    let mut map = std::collections::HashMap::with_capacity(rows.len());
    for r in rows {
        let id: String = r.get("id");
        let pb: String = r.get("poolBlockId");
        let pt: String = r.get("poolTxId");
        map.insert((pb, pt), id);
    }
    Ok(map)
}

/// Fetch token pairs (token0, token1) for a set of pool (block, tx) pairs.
/// Returns a map keyed by (poolBlockId, poolTxId) -> ((token0BlockId, token0TxId), (token1BlockId, token1TxId)).
pub async fn get_pool_tokens_for_pairs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pairs: &[(String, String)],
) -> Result<std::collections::HashMap<(String, String), ((String, String), (String, String))>> {
    if pairs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let block_ids: Vec<&str> = pairs.iter().map(|(pb, _)| pb.as_str()).collect();
    let tx_ids: Vec<&str> = pairs.iter().map(|(_, pt)| pt.as_str()).collect();
    let rows = sqlx::query(
        r#"select "poolBlockId", "poolTxId", "token0BlockId", "token0TxId", "token1BlockId", "token1TxId"
           from "Pool"
           where ("poolBlockId", "poolTxId") in (
                select pb, pt from unnest($1::text[], $2::text[]) as t(pb, pt)
           )"#
    )
    .bind(block_ids.as_slice())
    .bind(tx_ids.as_slice())
    .fetch_all(&mut **tx)
    .await?;

    let mut map = std::collections::HashMap::with_capacity(rows.len());
    for r in rows {
        let pb: String = r.get("poolBlockId");
        let pt: String = r.get("poolTxId");
        let t0b: String = r.get("token0BlockId");
        let t0t: String = r.get("token0TxId");
        let t1b: String = r.get("token1BlockId");
        let t1t: String = r.get("token1TxId");
        map.insert((pb, pt), ((t0b, t0t), (t1b, t1t)));
    }
    Ok(map)
}


