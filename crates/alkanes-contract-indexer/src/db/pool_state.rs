use anyhow::Result;
use sqlx::Row;

pub async fn get_latest_pool_states(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool_ids: &[&str],
) -> Result<std::collections::HashMap<String, (String, String, String)>> {
    if pool_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        r#"select distinct on ("poolId") "poolId", "token0Amount", "token1Amount", "tokenSupply"
           from "PoolState"
           where "poolId" = any($1)
           order by "poolId", "blockHeight" desc"#
    )
    .bind(pool_ids)
    .fetch_all(&mut **tx)
    .await?;

    let mut map = std::collections::HashMap::with_capacity(rows.len());
    for r in rows {
        let pid: String = r.get("poolId");
        let t0: String = r.get("token0Amount");
        let t1: String = r.get("token1Amount");
        let sup: String = r.get("tokenSupply");
        map.insert(pid, (t0, t1, sup));
    }
    Ok(map)
}

pub async fn insert_pool_state_snapshots(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    snapshots: &[(String, i32, String, String, String)],
) -> Result<()> {
    if snapshots.is_empty() { return Ok(()); }
    let mut q = String::from("insert into \"PoolState\" (\"poolId\", \"blockHeight\", \"token0Amount\", \"token1Amount\", \"tokenSupply\") values ");
    for i in 0..snapshots.len() {
        if i > 0 { q.push_str(","); }
        let base = i * 5;
        q.push_str(&format!("(${}, ${}, ${}, ${}, ${})", base+1, base+2, base+3, base+4, base+5));
    }
    q.push_str(" on conflict (\"poolId\", \"blockHeight\") do nothing");
    let mut qb = sqlx::query(&q);
    for (pid, bh, t0, t1, sup) in snapshots {
        qb = qb.bind(pid).bind(bh).bind(t0).bind(t1).bind(sup);
    }
    qb.execute(&mut **tx).await?;
    Ok(())
}


