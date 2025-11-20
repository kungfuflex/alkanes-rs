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
