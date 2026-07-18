use anyhow::Result;
use super::client::DataApiClient;
use crate::alkanes::types::AlkaneId;

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_alkanes(
    client: &DataApiClient,
    limit: Option<i32>,
    offset: Option<i32>,
    sort_by: Option<String>,
    order: Option<String>,
    search: Option<String>,
) -> Result<String> {
    let response = client.get_alkanes(limit, offset, sort_by, order, search).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_alkanes_by_address(
    client: &DataApiClient,
    address: &str,
) -> Result<String> {
    let alkanes = client.get_alkanes_by_address(address).await?;
    Ok(serde_json::to_string_pretty(&alkanes)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_alkane_details(
    client: &DataApiClient,
    id_str: &str,
) -> Result<String> {
    let id = parse_alkane_id(id_str)?;
    let alkane = client.get_alkane_details(&id).await?;
    Ok(serde_json::to_string_pretty(&alkane)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pools(
    client: &DataApiClient,
    factory_str: &str,
) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let pools = client.get_pools(&factory_id).await?;
    Ok(serde_json::to_string_pretty(&pools)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_by_id(
    client: &DataApiClient,
    id_str: &str,
) -> Result<String> {
    let id = parse_alkane_id(id_str)?;
    let pool = client.get_pool_by_id(&id).await?;
    Ok(serde_json::to_string_pretty(&pool)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_history(
    client: &DataApiClient,
    pool_id_str: &str,
    category: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = parse_alkane_id(pool_id_str)?;
    let history = client.get_pool_history(&pool_id, category, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&history)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_swap_history(
    client: &DataApiClient,
    pool_id_str: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = pool_id_str.as_ref().map(|s| parse_alkane_id(s)).transpose()?;
    let history = client.get_swap_history(pool_id.as_ref(), limit, offset).await?;
    Ok(serde_json::to_string_pretty(&history)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_bitcoin_price(client: &DataApiClient) -> Result<String> {
    let price = client.get_bitcoin_price().await?;
    Ok(serde_json::to_string_pretty(&price)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_market_chart(client: &DataApiClient, days: &str) -> Result<String> {
    let chart = client.get_bitcoin_market_chart(days).await?;
    Ok(serde_json::to_string_pretty(&chart)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_health(client: &DataApiClient) -> Result<String> {
    client.health().await?;
    Ok("OK".to_string())
}

pub fn parse_alkane_id(id_str: &str) -> Result<AlkaneId> {
    let parts: Vec<&str> = id_str.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid alkane ID format. Expected BLOCK:TX (e.g., 2:0)"
        ));
    }
    
    Ok(AlkaneId {
        block: parts[0].parse().map_err(|_| anyhow::anyhow!("Invalid block number"))?,
        tx: parts[1].parse().map_err(|_| anyhow::anyhow!("Invalid tx number"))?,
    })
}

// New Data API endpoints

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_balances(
    client: &DataApiClient,
    address: &str,
    include_outpoints: bool,
) -> Result<String> {
    let response = client.get_address_balances(address, include_outpoints).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_holders(
    client: &DataApiClient,
    alkane: &str,
    page: i64,
    limit: i64,
) -> Result<String> {
    let response = client.get_holders(alkane, page, limit).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_holders_count(
    client: &DataApiClient,
    alkane: &str,
) -> Result<String> {
    let response = client.get_holders_count(alkane).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_keys(
    client: &DataApiClient,
    alkane: &str,
    prefix: Option<String>,
    limit: i64,
) -> Result<String> {
    let response = client.get_keys(alkane, prefix, limit).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_trades(
    client: &DataApiClient,
    pool: &str,
    start_time: Option<i64>,
    end_time: Option<i64>,
    limit: i64,
) -> Result<String> {
    let response = client.get_trades(pool, start_time, end_time, limit).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_candles(
    client: &DataApiClient,
    pool: &str,
    interval: &str,
    start_time: Option<i64>,
    end_time: Option<i64>,
    limit: i64,
) -> Result<String> {
    let response = client.get_candles(pool, interval, start_time, end_time, limit).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_reserves(
    client: &DataApiClient,
    pool: &str,
) -> Result<String> {
    let response = client.get_reserves(pool).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Indexer status endpoints

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_block_height(client: &DataApiClient) -> Result<String> {
    let response = client.get_block_height().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_block_hash(client: &DataApiClient) -> Result<String> {
    let response = client.get_block_hash().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_indexer_position(client: &DataApiClient) -> Result<String> {
    let response = client.get_indexer_position().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Additional price endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_bitcoin_market_weekly(client: &DataApiClient) -> Result<String> {
    let response = client.get_bitcoin_market_weekly().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_bitcoin_markets(client: &DataApiClient) -> Result<String> {
    let response = client.get_bitcoin_markets().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Alkanes UTXO endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_alkanes_utxo(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_alkanes_utxo(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_amm_utxos(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_amm_utxos(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Search endpoint
#[cfg(feature = "std")]
pub async fn execute_dataapi_global_search(
    client: &DataApiClient,
    query: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.global_alkanes_search(query, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Address outpoints endpoint
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_outpoints(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_address_outpoints(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Pathfind endpoint
#[cfg(feature = "std")]
pub async fn execute_dataapi_pathfind(
    client: &DataApiClient,
    token_in: &str,
    token_out: &str,
    amount_in: &str,
    max_hops: Option<i32>,
) -> Result<String> {
    let response = client.pathfind(token_in, token_out, amount_in, max_hops).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Pool detail endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_details(client: &DataApiClient, pool_id_str: &str) -> Result<String> {
    let pool_id = parse_alkane_id(pool_id_str)?;
    let response = client.get_pool_details(&pool_id).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_pools_details(
    client: &DataApiClient,
    factory_str: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let response = client.get_all_pools_details(&factory_id, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Position endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_positions(client: &DataApiClient, address: &str, factory_str: &str) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let response = client.get_address_positions(address, &factory_id).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Token pairs endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_token_pairs(
    client: &DataApiClient,
    factory_str: &str,
    alkane_str: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let alkane_id = alkane_str.as_ref().map(|s| parse_alkane_id(s)).transpose()?;
    let response = client.get_token_pairs(&factory_id, alkane_id.as_ref(), limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_token_pairs(
    client: &DataApiClient,
    factory_str: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let response = client.get_all_token_pairs(&factory_id, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_alkane_swap_pair_details(
    client: &DataApiClient,
    factory_str: &str,
    token_a_str: &str,
    token_b_str: &str,
) -> Result<String> {
    let factory_id = parse_alkane_id(factory_str)?;
    let token_a_id = parse_alkane_id(token_a_str)?;
    let token_b_id = parse_alkane_id(token_b_str)?;
    let response = client.get_alkane_swap_pair_details(&factory_id, &token_a_id, &token_b_id).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Additional history endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_swap_history(
    client: &DataApiClient,
    pool_id_str: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = pool_id_str.as_ref().map(|s| parse_alkane_id(s)).transpose()?;
    let response = client.get_pool_swap_history(pool_id.as_ref(), limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_token_swap_history(
    client: &DataApiClient,
    alkane_str: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let alkane_id = parse_alkane_id(alkane_str)?;
    let response = client.get_token_swap_history(&alkane_id, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_mint_history(
    client: &DataApiClient,
    pool_id_str: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = pool_id_str.as_ref().map(|s| parse_alkane_id(s)).transpose()?;
    let response = client.get_pool_mint_history(pool_id.as_ref(), limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_pool_burn_history(
    client: &DataApiClient,
    pool_id_str: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = pool_id_str.as_ref().map(|s| parse_alkane_id(s)).transpose()?;
    let response = client.get_pool_burn_history(pool_id.as_ref(), limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Address-specific history endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_swap_history_for_pool(
    client: &DataApiClient,
    address: &str,
    pool_id_str: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let pool_id = parse_alkane_id(pool_id_str)?;
    let response = client.get_address_swap_history_for_pool(address, &pool_id, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_swap_history_for_token(
    client: &DataApiClient,
    address: &str,
    alkane_str: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let alkane_id = parse_alkane_id(alkane_str)?;
    let response = client.get_address_swap_history_for_token(address, &alkane_id, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Wrap/unwrap history endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_wrap_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_address_wrap_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_unwrap_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_address_unwrap_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_wrap_history(
    client: &DataApiClient,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_all_wrap_history(limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_unwrap_history(
    client: &DataApiClient,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_all_unwrap_history(limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_total_unwrap_amount(client: &DataApiClient) -> Result<String> {
    let response = client.get_total_unwrap_amount().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Address pool history endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_pool_creation_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_address_pool_creation_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_pool_mint_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_address_pool_mint_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_pool_burn_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_address_pool_burn_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// All AMM transaction history
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_address_amm_tx_history(
    client: &DataApiClient,
    address: &str,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_all_address_amm_tx_history(address, limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_all_amm_tx_history(
    client: &DataApiClient,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<String> {
    let response = client.get_all_amm_tx_history(limit, offset).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

// Bitcoin/UTXO endpoints
#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_balance(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_address_balance(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_taproot_balance(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_taproot_balance(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_address_utxos(client: &DataApiClient, address: &str) -> Result<String> {
    let response = client.get_address_utxos(address).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_account_utxos(client: &DataApiClient, account: &str) -> Result<String> {
    let response = client.get_account_utxos(account).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_account_balance(client: &DataApiClient, account: &str) -> Result<String> {
    let response = client.get_account_balance(account).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_taproot_history(
    client: &DataApiClient,
    taproot_address: &str,
    total_txs: i32,
) -> Result<String> {
    let response = client.get_taproot_history(taproot_address, total_txs).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_dataapi_get_intent_history(
    client: &DataApiClient,
    address: &str,
    total_txs: Option<i32>,
    last_seen_tx_id: Option<&str>,
) -> Result<String> {
    let response = client.get_intent_history(address, total_txs, last_seen_tx_id).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}
