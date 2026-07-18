use anyhow::Result;
use redis::Client;

pub fn create_client(redis_url: &str) -> Result<Client> {
    let client = Client::open(redis_url)?;
    Ok(client)
}
