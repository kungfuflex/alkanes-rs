use anyhow::Result;
use deezel_common::provider::ConcreteProvider;

pub async fn build_provider(
    bitcoin_rpc_url: Option<String>,
    sandshrew_rpc_url: String,
    esplora_url: Option<String>,
    network_provider: String,
) -> Result<ConcreteProvider> {
    let provider = ConcreteProvider::new(
        bitcoin_rpc_url,
        sandshrew_rpc_url.clone(),
        Some(sandshrew_rpc_url),
        esplora_url,
        network_provider,
        #[cfg(not(target_arch = "wasm32"))]
        None,
        #[cfg(target_arch = "wasm32")]
        None,
    )
    .await?;
    Ok(provider)
}


