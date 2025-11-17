use anyhow::Result;
use chrono::{DateTime, Utc};

/// Ensure the "ProcessedBlocks" table exists (idempotent).
pub async fn ensure_processed_blocks_table(pool: &sqlx::PgPool) -> Result<()> {
    sqlx::query(
        r#"
        create table if not exists "ProcessedBlocks" (
          "blockHeight" integer not null unique,
          "blockHash" text not null unique,
          "timestamp" timestamptz not null,
          "isProcessing" boolean not null default false,
          "createdAt" timestamptz not null default now()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        create index if not exists "idx_ProcessedBlocks_blockHash" on "ProcessedBlocks"("blockHash")
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert a row into the "ProcessedBlocks" table after a block is fully processed.
/// - blockHeight: unique per row
/// - blockHash: unique per row
/// - timestamp: block timestamp (or now if unknown)
/// Sets isProcessing=false on upsert.
pub async fn upsert_processed_block(
    pool: &sqlx::PgPool,
    block_height: i32,
    block_hash: &str,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into "ProcessedBlocks" ("blockHeight", "blockHash", "timestamp", "isProcessing")
        values ($1, $2, $3, false)
        on conflict ("blockHeight") do update set
            "blockHash" = excluded."blockHash",
            "timestamp" = excluded."timestamp",
            "isProcessing" = false
        "#,
    )
    .bind(block_height)
    .bind(block_hash)
    .bind(timestamp)
    .execute(pool)
    .await?;
    Ok(())
}


