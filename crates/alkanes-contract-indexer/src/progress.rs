use anyhow::Result;
use sqlx::PgPool;

pub struct ProgressStore {
    pool: PgPool,
}

impl ProgressStore {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    // Returns last fully processed height for sequential processors
    pub async fn get_last_processed_height(&self) -> Result<Option<u64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "select value::bigint from kv_store where key = 'last_processed_height'"
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(v,)| v as u64))
    }

    pub async fn set_last_processed_height(&self, height: u64) -> Result<()> {
        sqlx::query(
            "insert into kv_store(key, value) values ('last_processed_height', $1)
             on conflict (key) do update set value = excluded.value",
        )
        .bind(height as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

// Minimal bootstrap DDL helper (optional; call from migrations in real setup)
pub async fn ensure_kv_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "create table if not exists kv_store (
            key text primary key,
            value text not null
        )"
    )
    .execute(pool)
    .await?;
    Ok(())
}


