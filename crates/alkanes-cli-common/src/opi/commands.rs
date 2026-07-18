/// OPI command execution functions
use anyhow::Result;
use super::client::OpiClient;

#[cfg(feature = "std")]
pub async fn execute_opi_block_height(client: &OpiClient) -> Result<String> {
    let height = client.get_block_height().await?;
    match height {
        Some(h) => Ok(h.to_string()),
        None => Ok("null".to_string()),
    }
}

#[cfg(feature = "std")]
pub async fn execute_opi_extras_block_height(client: &OpiClient) -> Result<String> {
    let height = client.get_extras_block_height().await?;
    match height {
        Some(h) => Ok(h.to_string()),
        None => Ok("null".to_string()),
    }
}

#[cfg(feature = "std")]
pub async fn execute_opi_db_version(client: &OpiClient) -> Result<String> {
    client.get_db_version().await
}

#[cfg(feature = "std")]
pub async fn execute_opi_event_hash_version(client: &OpiClient) -> Result<String> {
    client.get_event_hash_version().await
}

#[cfg(feature = "std")]
pub async fn execute_opi_balance_on_block(
    client: &OpiClient,
    block_height: u64,
    pkscript: &str,
    ticker: &str,
) -> Result<String> {
    let response = client.get_balance_on_block(block_height, pkscript, ticker).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_activity_on_block(
    client: &OpiClient,
    block_height: u64,
) -> Result<String> {
    let response = client.get_activity_on_block(block_height).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_bitcoin_rpc_results_on_block(
    client: &OpiClient,
    block_height: u64,
) -> Result<String> {
    let response = client.get_bitcoin_rpc_results_on_block(block_height).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_current_balance(
    client: &OpiClient,
    ticker: &str,
    address: Option<&str>,
    pkscript: Option<&str>,
) -> Result<String> {
    let response = client.get_current_balance_of_wallet(ticker, address, pkscript).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_valid_tx_notes_of_wallet(
    client: &OpiClient,
    address: Option<&str>,
    pkscript: Option<&str>,
) -> Result<String> {
    let response = client.get_valid_tx_notes_of_wallet(address, pkscript).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_valid_tx_notes_of_ticker(
    client: &OpiClient,
    ticker: &str,
) -> Result<String> {
    let response = client.get_valid_tx_notes_of_ticker(ticker).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_holders(
    client: &OpiClient,
    ticker: &str,
) -> Result<String> {
    let response = client.get_holders(ticker).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_hash_of_all_activity(
    client: &OpiClient,
    block_height: u64,
) -> Result<String> {
    let response = client.get_hash_of_all_activity(block_height).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_hash_of_all_current_balances(client: &OpiClient) -> Result<String> {
    let response = client.get_hash_of_all_current_balances().await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_event(
    client: &OpiClient,
    inscription_id: &str,
) -> Result<String> {
    let response = client.get_event(inscription_id).await?;
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(feature = "std")]
pub async fn execute_opi_ip(client: &OpiClient) -> Result<String> {
    client.get_ip().await
}

#[cfg(feature = "std")]
pub async fn execute_opi_raw(client: &OpiClient, endpoint: &str) -> Result<String> {
    client.get_raw(endpoint).await
}
