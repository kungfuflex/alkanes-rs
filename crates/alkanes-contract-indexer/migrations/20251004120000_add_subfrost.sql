-- Add Subfrost wrap/unwrap event tables (non-destructive)

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


