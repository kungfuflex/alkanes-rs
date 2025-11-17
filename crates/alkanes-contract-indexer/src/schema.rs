use anyhow::Result;
use sqlx::PgPool;

// This DDL mirrors the provided Prisma models as closely as possible in Postgres.
// Types:
// - Prisma String -> text/uuid
// - Json -> jsonb
// - DateTime -> timestamptz

pub const DDL: &str = r#"
-- Ensure UUID generation is available
create extension if not exists pgcrypto;

create table if not exists "AlkaneTransaction" (
  "transactionId" text primary key,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "hasTrace" boolean not null default false,
  "traceSucceed" boolean not null default false,
  "transactionData" jsonb,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);

create index if not exists "idx_AlkaneTransaction_blockHeight_transactionIndex" on "AlkaneTransaction"("blockHeight", "transactionIndex");
create index if not exists "idx_AlkaneTransaction_brin_blockHeight" on "AlkaneTransaction" using brin ("blockHeight") with (pages_per_range = 128);
alter table "AlkaneTransaction" alter column "transactionData" set storage external;

create table if not exists "TraceEvent" (
  "id" uuid primary key default gen_random_uuid(),
  "transactionId" text not null,
  "vout" integer not null,
  "blockHeight" integer not null,
  "alkaneAddressBlock" text not null,
  "alkaneAddressTx" text not null,
  "eventType" text not null,
  "data" jsonb not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  constraint "fk_TraceEvent_transaction" foreign key ("transactionId") references "AlkaneTransaction"("transactionId")
);

create index if not exists "idx_TraceEvent_transactionId" on "TraceEvent"("transactionId");
create index if not exists "idx_TraceEvent_eventType" on "TraceEvent"("eventType");
create index if not exists "idx_TraceEvent_brin_blockHeight" on "TraceEvent" using brin ("blockHeight") with (pages_per_range = 128);
create index if not exists "idx_TraceEvent_blockHeight_eventType" on "TraceEvent"("blockHeight", "eventType");
alter table "TraceEvent" set (fillfactor = 80, autovacuum_vacuum_scale_factor = 0.01, autovacuum_vacuum_threshold = 5000, autovacuum_analyze_scale_factor = 0.02);
alter table "TraceEvent" alter column "data" set storage external;

create table if not exists "DecodedProtostone" (
  "transactionId" text not null,
  "vout" integer not null,
  "protostoneIndex" integer not null,
  "blockHeight" integer not null,
  "decoded" jsonb not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  constraint "DecodedProtostone_pkey" primary key ("transactionId", "vout", "protostoneIndex"),
  constraint "fk_DecodedProtostone_transaction" foreign key ("transactionId") references "AlkaneTransaction"("transactionId")
);

create index if not exists "idx_DecodedProtostone_brin_blockHeight" on "DecodedProtostone" using brin ("blockHeight") with (pages_per_range = 128);
alter table "DecodedProtostone" set (fillfactor = 80, autovacuum_vacuum_scale_factor = 0.01, autovacuum_vacuum_threshold = 5000, autovacuum_analyze_scale_factor = 0.02);
alter table "DecodedProtostone" alter column "decoded" set storage external;

create table if not exists "ClockIn" (
  "id" uuid primary key default gen_random_uuid(),
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "userAddress" text not null,
  "timestamp" timestamptz not null,
  "oylPayment" boolean not null default false,
  "paymentVout" integer,
  "paymentAmount" integer,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  constraint "fk_ClockIn_transaction" foreign key ("transactionId") references "AlkaneTransaction"("transactionId")
);

create index if not exists "idx_ClockIn_transactionId" on "ClockIn"("transactionId");
create index if not exists "idx_ClockIn_blockHeight" on "ClockIn"("blockHeight");
create index if not exists "idx_ClockIn_userAddress" on "ClockIn"("userAddress");
create index if not exists "idx_ClockIn_blockHeight_transactionIndex" on "ClockIn"("blockHeight", "transactionIndex");

create table if not exists "ProcessedBlocks" (
  "blockHeight" integer not null unique,
  "blockHash" text not null unique,
  "timestamp" timestamptz not null,
  "isProcessing" boolean not null default false,
  "createdAt" timestamptz not null default now()
);

create index if not exists "idx_ProcessedBlocks_blockHash" on "ProcessedBlocks"("blockHash");

create table if not exists "ClockInBlockSummary" (
  "id" text primary key default gen_random_uuid()::text,
  "blockHeight" integer not null unique,
  "timestamp" timestamptz not null,
  "totalClockIns" integer not null default 0,
  "uniqueUsers" integer not null default 0,
  "isEligibleBlock" boolean not null default false,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);

create index if not exists "idx_ClockInBlockSummary_blockHeight" on "ClockInBlockSummary"("blockHeight");

create table if not exists "ClockInSummary" (
  "userAddress" text primary key,
  "currentStreak" integer not null default 0,
  "maxStreak" integer not null default 0,
  "totalCount" integer not null default 0,
  "lastClockInBlock" integer,
  "lastClockInTimestamp" timestamptz,
  "firstClockInBlock" integer,
  "firstClockInTimestamp" timestamptz,
  "empCount" integer not null default 0,
  "vstrCount" integer not null default 0,
  "empCurrentStreak" integer not null default 0,
  "vstrCurrentStreak" integer not null default 0,
  "empMaxStreak" integer not null default 0,
  "vstrMaxStreak" integer not null default 0,
  "empNumber" integer,
  "vstrNumber" integer,
  "updatedAt" timestamptz not null default now()
);

create index if not exists "idx_ClockInSummary_totalCount" on "ClockInSummary"("totalCount");
create index if not exists "idx_ClockInSummary_currentStreak" on "ClockInSummary"("currentStreak");
create index if not exists "idx_ClockInSummary_maxStreak" on "ClockInSummary"("maxStreak");
create index if not exists "idx_ClockInSummary_empCount" on "ClockInSummary"("empCount");
create index if not exists "idx_ClockInSummary_vstrCount" on "ClockInSummary"("vstrCount");
create index if not exists "idx_ClockInSummary_empCurrentStreak" on "ClockInSummary"("empCurrentStreak");
create index if not exists "idx_ClockInSummary_vstrCurrentStreak" on "ClockInSummary"("vstrCurrentStreak");
create index if not exists "idx_ClockInSummary_empNumber" on "ClockInSummary"("empNumber");
create index if not exists "idx_ClockInSummary_vstrNumber" on "ClockInSummary"("vstrNumber");
create index if not exists "idx_ClockInSummary_lastClockInTimestamp" on "ClockInSummary"("lastClockInTimestamp");

create table if not exists "CorpData" (
  "id" uuid primary key default gen_random_uuid(),
  "empCount" integer not null default 0,
  "vstrCount" integer not null default 0,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);

create table if not exists "Profile" (
  "id" uuid primary key default gen_random_uuid(),
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  "userAddress" text not null unique,
  "twitterAvatarUrl" text not null default '',
  "twitterUsername" text not null default ''
);
create index if not exists "idx_Profile_userAddress" on "Profile"("userAddress");

create table if not exists "Pool" (
  "id" text primary key default gen_random_uuid()::text,
  "factoryBlockId" text not null,
  "factoryTxId" text not null,
  "poolBlockId" text not null,
  "poolTxId" text not null,
  "token0BlockId" text not null,
  "token0TxId" text not null,
  "token1BlockId" text not null,
  "token1TxId" text not null,
  "poolName" text not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  constraint "uq_Pool_poolBlockId_poolTxId" unique ("poolBlockId", "poolTxId")
);
create index if not exists "idx_Pool_factoryBlockId_factoryTxId" on "Pool"("factoryBlockId", "factoryTxId");
create index if not exists "idx_Pool_token0_token1" on "Pool"("token0BlockId", "token0TxId", "token1BlockId", "token1TxId");
create index if not exists "idx_Pool_token1_token0" on "Pool"("token1BlockId", "token1TxId", "token0BlockId", "token0TxId");

create table if not exists "PoolState" (
  "id" text primary key default gen_random_uuid()::text,
  "poolId" text not null,
  "blockHeight" integer not null,
  "token0Amount" text not null,
  "token1Amount" text not null,
  "tokenSupply" text not null,
  "createdAt" timestamptz not null default now(),
  constraint "fk_PoolState_pool" foreign key ("poolId") references "Pool"("id") on delete cascade,
  constraint "uq_PoolState_poolId_blockHeight" unique ("poolId", "blockHeight")
);
create index if not exists "idx_PoolState_poolId" on "PoolState"("poolId");
create index if not exists "idx_PoolState_blockHeight" on "PoolState"("blockHeight");

create table if not exists "PoolCreation" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "poolBlockId" text not null,
  "poolTxId" text not null,
  "token0BlockId" text not null,
  "token0TxId" text not null,
  "token1BlockId" text not null,
  "token1TxId" text not null,
  "token0Amount" text not null,
  "token1Amount" text not null,
  "tokenSupply" text not null,
  "creatorAddress" text,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  constraint "fk_PoolCreation_pool" foreign key ("poolBlockId", "poolTxId") references "Pool"("poolBlockId", "poolTxId")
);
create unique index if not exists "uq_PoolCreation_poolBlockId_poolTxId" on "PoolCreation"("poolBlockId", "poolTxId");
create index if not exists "idx_PoolCreation_transactionId" on "PoolCreation"("transactionId");
create index if not exists "idx_PoolCreation_blockHeight" on "PoolCreation"("blockHeight");
create index if not exists "idx_PoolCreation_poolBlockId_poolTxId" on "PoolCreation"("poolBlockId", "poolTxId");
create index if not exists "idx_PoolCreation_blockHeight_transactionIndex" on "PoolCreation"("blockHeight", "transactionIndex");
create index if not exists "idx_PoolCreation_success_block_tx" on "PoolCreation"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_PoolCreation_pool_ts" on "PoolCreation"("poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolCreation_creator_ts" on "PoolCreation"("creatorAddress", "timestamp");
create index if not exists "idx_PoolCreation_creator_pool_ts" on "PoolCreation"("creatorAddress", "poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolCreation_brin_timestamp" on "PoolCreation" using brin ("timestamp") with (pages_per_range = 128);

create table if not exists "PoolSwap" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "poolBlockId" text not null,
  "poolTxId" text not null,
  "soldTokenBlockId" text not null,
  "soldTokenTxId" text not null,
  "boughtTokenBlockId" text not null,
  "boughtTokenTxId" text not null,
  "soldAmount" double precision not null,
  "boughtAmount" double precision not null,
  "sellerAddress" text,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);
create index if not exists "idx_PoolSwap_transactionId" on "PoolSwap"("transactionId");
create index if not exists "idx_PoolSwap_blockHeight" on "PoolSwap"("blockHeight");
create index if not exists "idx_PoolSwap_poolBlockId_poolTxId" on "PoolSwap"("poolBlockId", "poolTxId");
create index if not exists "idx_PoolSwap_blockHeight_transactionIndex" on "PoolSwap"("blockHeight", "transactionIndex");
create index if not exists "idx_PoolSwap_success_block_tx" on "PoolSwap"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_PoolSwap_pool_ts" on "PoolSwap"("poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolSwap_soldToken_ts" on "PoolSwap"("soldTokenBlockId", "soldTokenTxId", "timestamp");
create index if not exists "idx_PoolSwap_boughtToken_ts" on "PoolSwap"("boughtTokenBlockId", "boughtTokenTxId", "timestamp");
create index if not exists "idx_PoolSwap_seller_ts" on "PoolSwap"("sellerAddress", "timestamp");
create index if not exists "idx_PoolSwap_seller_pool_ts" on "PoolSwap"("sellerAddress", "poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolSwap_seller_soldToken_ts" on "PoolSwap"("sellerAddress", "soldTokenBlockId", "soldTokenTxId", "timestamp");
create index if not exists "idx_PoolSwap_seller_boughtToken_ts" on "PoolSwap"("sellerAddress", "boughtTokenBlockId", "boughtTokenTxId", "timestamp");
create index if not exists "idx_PoolSwap_brin_timestamp" on "PoolSwap" using brin ("timestamp") with (pages_per_range = 128);

create table if not exists "PoolBurn" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "poolBlockId" text not null,
  "poolTxId" text not null,
  "lpTokenAmount" text not null,
  "token0BlockId" text not null,
  "token0TxId" text not null,
  "token1BlockId" text not null,
  "token1TxId" text not null,
  "token0Amount" text not null,
  "token1Amount" text not null,
  "burnerAddress" text,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);
create index if not exists "idx_PoolBurn_transactionId" on "PoolBurn"("transactionId");
create index if not exists "idx_PoolBurn_blockHeight" on "PoolBurn"("blockHeight");
create index if not exists "idx_PoolBurn_poolBlockId_poolTxId" on "PoolBurn"("poolBlockId", "poolTxId");
create index if not exists "idx_PoolBurn_blockHeight_transactionIndex" on "PoolBurn"("blockHeight", "transactionIndex");
create index if not exists "idx_PoolBurn_success_block_tx" on "PoolBurn"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_PoolBurn_pool_ts" on "PoolBurn"("poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolBurn_burner_ts" on "PoolBurn"("burnerAddress", "timestamp");
create index if not exists "idx_PoolBurn_burner_pool_ts" on "PoolBurn"("burnerAddress", "poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolBurn_brin_timestamp" on "PoolBurn" using brin ("timestamp") with (pages_per_range = 128);

create table if not exists "PoolMint" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "poolBlockId" text not null,
  "poolTxId" text not null,
  "lpTokenAmount" text not null,
  "token0BlockId" text not null,
  "token0TxId" text not null,
  "token1BlockId" text not null,
  "token1TxId" text not null,
  "token0Amount" text not null,
  "token1Amount" text not null,
  "minterAddress" text,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);
create index if not exists "idx_PoolMint_transactionId" on "PoolMint"("transactionId");
create index if not exists "idx_PoolMint_blockHeight" on "PoolMint"("blockHeight");
create index if not exists "idx_PoolMint_poolBlockId_poolTxId" on "PoolMint"("poolBlockId", "poolTxId");
create index if not exists "idx_PoolMint_blockHeight_transactionIndex" on "PoolMint"("blockHeight", "transactionIndex");
create index if not exists "idx_PoolMint_success_block_tx" on "PoolMint"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_PoolMint_pool_ts" on "PoolMint"("poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolMint_minter_ts" on "PoolMint"("minterAddress", "timestamp");
create index if not exists "idx_PoolMint_minter_pool_ts" on "PoolMint"("minterAddress", "poolBlockId", "poolTxId", "timestamp");
create index if not exists "idx_PoolMint_brin_timestamp" on "PoolMint" using brin ("timestamp") with (pages_per_range = 128);

create table if not exists "CuratedPools" (
  "id" text primary key default gen_random_uuid()::text,
  "factoryId" text not null unique,
  "poolIds" text[] not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);

-- Subfrost wrap/unwrap events
create table if not exists "SubfrostWrap" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "address" text,
  "amount" text not null,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);
create index if not exists "idx_SubfrostWrap_transactionId" on "SubfrostWrap"("transactionId");
create index if not exists "idx_SubfrostWrap_blockHeight" on "SubfrostWrap"("blockHeight");
create index if not exists "idx_SubfrostWrap_address_ts" on "SubfrostWrap"("address", "timestamp");
create index if not exists "idx_SubfrostWrap_blockHeight_transactionIndex" on "SubfrostWrap"("blockHeight", "transactionIndex");
create index if not exists "idx_SubfrostWrap_success_block_tx" on "SubfrostWrap"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_SubfrostWrap_brin_timestamp" on "SubfrostWrap" using brin ("timestamp") with (pages_per_range = 128);

create table if not exists "SubfrostUnwrap" (
  "id" text primary key default gen_random_uuid()::text,
  "transactionId" text not null,
  "blockHeight" integer not null,
  "transactionIndex" integer not null default 0,
  "address" text,
  "amount" text not null,
  "successful" boolean not null default true,
  "timestamp" timestamptz not null,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now()
);
create index if not exists "idx_SubfrostUnwrap_transactionId" on "SubfrostUnwrap"("transactionId");
create index if not exists "idx_SubfrostUnwrap_blockHeight" on "SubfrostUnwrap"("blockHeight");
create index if not exists "idx_SubfrostUnwrap_address_ts" on "SubfrostUnwrap"("address", "timestamp");
create index if not exists "idx_SubfrostUnwrap_blockHeight_transactionIndex" on "SubfrostUnwrap"("blockHeight", "transactionIndex");
create index if not exists "idx_SubfrostUnwrap_success_block_tx" on "SubfrostUnwrap"("successful", "blockHeight", "transactionIndex");
create index if not exists "idx_SubfrostUnwrap_brin_timestamp" on "SubfrostUnwrap" using brin ("timestamp") with (pages_per_range = 128);

-- progress KV store (already used by coordinator)
create table if not exists kv_store (
  key text primary key,
  value text not null
);

-- backfill columns for existing deployments
alter table "PoolCreation" add column if not exists "successful" boolean not null default true;
alter table "PoolSwap" add column if not exists "successful" boolean not null default true;
alter table "PoolBurn" add column if not exists "successful" boolean not null default true;
alter table "PoolMint" add column if not exists "successful" boolean not null default true;
"#;

const DROP_ALL: &str = r#"
drop table if exists "PoolMint" cascade;
drop table if exists "PoolBurn" cascade;
drop table if exists "PoolSwap" cascade;
drop table if exists "PoolCreation" cascade;
drop table if exists "PoolState" cascade;
drop table if exists "Pool" cascade;
drop table if exists "Profile" cascade;
drop table if exists "CorpData" cascade;
drop table if exists "ClockInSummary" cascade;
drop table if exists "ClockInBlockSummary" cascade;
drop table if exists "ProcessedBlocks" cascade;
drop table if exists "ClockIn" cascade;
drop table if exists "TraceEvent" cascade;
drop table if exists "DecodedProtostone" cascade;
drop table if exists "AlkaneTransaction" cascade;
drop table if exists "CuratedPools" cascade;
drop table if exists "SubfrostUnwrap" cascade;
drop table if exists "SubfrostWrap" cascade;
drop table if exists kv_store cascade;
"#;

async fn execute_batch(pool: &PgPool, sql: &str) -> Result<()> {
    for stmt in sql.split(';') {
        let s = stmt.trim();
        if s.is_empty() { continue; }
        sqlx::query(s).execute(pool).await?;
    }
    Ok(())
}

pub async fn push_schema(pool: &PgPool) -> Result<()> {
    execute_batch(pool, DDL).await
}

pub async fn reset_schema(pool: &PgPool) -> Result<()> {
    // Drop known tables, then re-push
    execute_batch(pool, DROP_ALL).await?;
    push_schema(pool).await
}

pub async fn drop_all_tables(pool: &PgPool) -> Result<()> {
    execute_batch(pool, DROP_ALL).await
}


