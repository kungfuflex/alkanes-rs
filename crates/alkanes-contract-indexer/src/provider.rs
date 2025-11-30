use anyhow::Result;
use alkanes_cli_sys::SystemAlkanes as ConcreteProvider;
use alkanes_cli_common::commands::Args;
use alkanes_cli_common::network::RpcConfig;

pub async fn build_provider(
    bitcoin_rpc_url: Option<String>,
    sandshrew_rpc_url: String,
    esplora_url: Option<String>,
    network_provider: String,
) -> Result<ConcreteProvider> {
    // Build Args using the RpcConfig
    let rpc_config = RpcConfig {
        sandshrew_rpc_url: Some(sandshrew_rpc_url.clone()),
        bitcoin_rpc_url,
        metashrew_rpc_url: Some(sandshrew_rpc_url.clone()),
        esplora_url,
        ord_url: None,
        titan_api_url: None,
        brc20_prog_rpc_url: None,  // Not used by indexer
        jsonrpc_url: Some(sandshrew_rpc_url),  // For AlkanesProvider::trace
        subfrost_api_key: None,  // Not needed for indexer
        provider: network_provider,
        timeout_seconds: 600,
    };
    
    let args = Args {
        rpc_config,
        magic: None,
        wallet_file: None,
        passphrase: None,
        hd_path: None,
        wallet_address: None,
        wallet_key: None,
        wallet_key_file: None,
        brc20_prog_rpc_url: None,
        log_level: "info".to_string(),
        command: alkanes_cli_common::commands::Commands::Metashrew {
            command: alkanes_cli_common::commands::MetashrewCommands::Height,
        },
    };
    
    let provider = ConcreteProvider::new(&args).await?;
    Ok(provider)
}


