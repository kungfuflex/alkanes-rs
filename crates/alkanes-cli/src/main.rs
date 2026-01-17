//! ALKANES CLI - A thin wrapper around the alkanes-cli-sys library
//!
//! This crate is responsible for parsing command-line arguments and delegating
//! the actual work to the alkanes-cli-sys library. This keeps the CLI crate
//! lightweight and focused on its primary role as a user interface.

use anyhow::Result;
use clap::Parser;
use alkanes_cli_sys::{SystemAlkanes, SystemOrd};
use alkanes_cli_common::traits::*;
use futures::future::join_all;
use serde_json::json;

mod commands;
mod pretty_print;
mod format_parser;
use commands::{Alkanes, AlkanesExecute, Commands, DeezelCommands, MetashrewCommands, Protorunes, Runestone, WalletCommands, DataApiCommand, SubfrostCommands, OpiCommands};
use alkanes_cli_common::alkanes;
use pretty_print::*;

/// Resolve address identifier (e.g., "p2tr:0") to actual address if needed
/// If the address doesn't contain identifier patterns, returns it as-is
async fn resolve_address_identifier(args: &DeezelCommands, address: &str) -> Result<String> {
    // Check if address contains identifier patterns like "p2tr:", "p2wsh:", etc.
    let is_identifier = address.contains("p2tr:") ||
                        address.contains("p2wsh:") ||
                        address.contains("p2wpkh:") ||
                        address.contains("p2sh:");

    if !is_identifier {
        // Not an identifier, return as-is (assume it's a raw address)
        return Ok(address.to_string());
    }

    // Need to create a system to resolve the identifier
    let alkanes_args = alkanes_cli_common::commands::Args::from(args);
    let system = SystemAlkanes::new(&alkanes_args).await?;
    let resolved = system.provider().resolve_all_identifiers(address).await?;
    Ok(resolved)
}

/// Parse a BTC amount string and convert to satoshis
/// Accepts formats like "0.0001", "1.5", "0.00000001", etc.
fn parse_btc_amount(amount_str: &str) -> Result<u64> {
    let amount_f64: f64 = amount_str.parse()
        .map_err(|_| anyhow::anyhow!("Invalid amount format: '{}'. Expected decimal BTC amount (e.g., 0.0001)", amount_str))?;
    
    if amount_f64 < 0.0 {
        return Err(anyhow::anyhow!("Amount cannot be negative"));
    }
    
    // Convert BTC to satoshis (1 BTC = 100,000,000 satoshis)
    let satoshis = (amount_f64 * 100_000_000.0).round() as u64;
    
    if satoshis == 0 && amount_f64 > 0.0 {
        return Err(anyhow::anyhow!("Amount too small: minimum is 0.00000001 BTC (1 satoshi)"));
    }
    
    Ok(satoshis)
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = DeezelCommands::parse();

    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    // Handle keystore logic

    // Handle Dataapi commands early (they don't need the System trait)
    if let Commands::Dataapi(ref cmd) = args.command {
        return execute_dataapi_command(&args, cmd.clone()).await;
    }

    // Handle OPI commands early (they don't need the System trait)
    if let Commands::Opi(ref cmd) = args.command {
        return execute_opi_command(&args, cmd.clone()).await;
    }

    // Convert DeezelCommands to Args
    let alkanes_args = alkanes_cli_common::commands::Args::from(&args);

    // Validate RPC config (ensure only one backend is configured)
    alkanes_args.rpc_config.validate()?;

    // Check if this command needs wallet access
    // IMPORTANT: If --wallet-file is provided, always load the wallet (even in Locked state)
    // because address identifiers like "p2tr:0" need wallet access for address derivation
    let skip_wallet_init = !args.command.requires_wallet() && alkanes_args.wallet_file.is_none();

    // Create a new SystemAlkanes instance (skip wallet init only if not needed AND no wallet file provided)
    let mut system = SystemAlkanes::new_with_options(&alkanes_args, skip_wallet_init).await?;

    // Set default brc20-prog RPC URL based on network if not provided
    let brc20_prog_rpc_url = alkanes_args.brc20_prog_rpc_url.clone()
        .or_else(|| alkanes_args.rpc_config.get_default_brc20_prog_rpc_url());

    // Get JSON-RPC headers (used by brc20-prog RPC client)
    let jsonrpc_headers = alkanes_args.rpc_config.get_jsonrpc_headers();

    // Execute other commands
    execute_command(&mut system, args.command, brc20_prog_rpc_url, jsonrpc_headers, args.frbtc_address.clone()).await
}

async fn execute_command<T: System + SystemOrd + UtxoProvider>(system: &mut T, command: Commands, brc20_prog_rpc_url: Option<String>, jsonrpc_headers: Vec<(String, String)>, frbtc_address: Option<String>) -> Result<()> {
    match command {
        Commands::Bitcoind(cmd) => system.execute_bitcoind_command(cmd.into()).await.map_err(|e| e.into()),
        Commands::Wallet(cmd) => execute_wallet_command(system, cmd).await,
        Commands::Alkanes(cmd) => execute_alkanes_command(system, cmd).await,
        Commands::Runestone(cmd) => execute_runestone_command(system, cmd).await,
        Commands::Protorunes(cmd) => execute_protorunes_command(system.provider(), cmd).await,
        Commands::Ord(cmd) => execute_ord_command(system.provider(), cmd.into()).await,
        Commands::Esplora(cmd) => execute_esplora_command(system.provider(), cmd.into()).await,
        Commands::Metashrew(cmd) => execute_metashrew_command(system.provider(), cmd).await,
        Commands::Lua(cmd) => execute_lua_command(system.provider(), cmd).await,
        Commands::Brc20Prog(cmd) => execute_brc20prog_command(system, cmd, brc20_prog_rpc_url, jsonrpc_headers, frbtc_address).await,
        Commands::Dataapi(_) => {
            // Dataapi is handled in main() because it doesn't need the System trait
            unreachable!("Dataapi commands should be handled in main()")
        }
        Commands::Opi(_) => {
            // OPI is handled in main() because it doesn't need the System trait
            unreachable!("OPI commands should be handled in main()")
        }
        Commands::Subfrost(cmd) => execute_subfrost_command(system.provider(), cmd).await,
        Commands::Espo(cmd) => execute_espo_command(system.provider(), cmd.into()).await,
        Commands::Decodepsbt { psbt, raw } => {
            use alkanes_cli_common::psbt_utils::decode_psbt_from_base64;
            let psbt_json = decode_psbt_from_base64(&psbt)?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&psbt_json)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&psbt_json)?);
            }
            Ok(())
        }
    }
}

async fn execute_lua_command(
    provider: &dyn DeezelProvider,
    command: crate::commands::LuaCommands,
) -> anyhow::Result<()> {
    use crate::commands::LuaCommands;
    use alkanes_cli_common::lua_script::{LuaScript, LuaScriptExecutor};
    use std::fs;

    match command {
        LuaCommands::Evalscript { script, args, raw } => {
            let script_content = fs::read_to_string(&script)
                .map_err(|e| anyhow::anyhow!("Failed to read script file {}: {}", script, e))?;

            let lua_script = LuaScript::from_string(script_content);

            if !raw {
                println!("Script hash: {}", lua_script.hash());
            }

            // Resolve args (convert identifiers like alkane IDs to their values)
            let mut resolved_args = Vec::new();
            for arg in args {
                match provider.resolve_all_identifiers(&arg).await {
                    Ok(resolved) => resolved_args.push(serde_json::Value::String(resolved)),
                    Err(_) => resolved_args.push(serde_json::Value::String(arg)),
                }
            }

            // Execute the script (tries evalsaved first, falls back to evalscript)
            let result = provider.execute_lua_script(&lua_script, resolved_args).await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

async fn execute_subfrost_command(
    provider: &dyn DeezelProvider,
    command: SubfrostCommands,
) -> Result<()> {
    use alkanes_cli_common::subfrost;

    match command {
        SubfrostCommands::MinimumUnwrap {
            fee_rate,
            premium,
            expected_inputs,
            expected_outputs,
            raw,
        } => {
            let result = subfrost::execute_minimum_unwrap(
                provider,
                fee_rate,
                premium,
                expected_inputs,
                expected_outputs,
                raw,
            )
            .await?;
            println!("{}", result);
        }
        SubfrostCommands::Thieve {
            address,
            amount,
            raw,
        } => {
            let result = subfrost::execute_thieve(
                provider,
                &address,
                amount,
                raw,
            )
            .await?;
            println!("{}", result);
        }
    }
    Ok(())
}

async fn execute_dataapi_command(args: &DeezelCommands, command: DataApiCommand) -> Result<()> {
    use alkanes_cli_common::dataapi::DataApiClient;
    
    // Determine the data API URL based on --data-api flag or provider network
    let api_url = if let Some(ref url) = args.data_api {
        url.clone()
    } else {
        match args.provider.as_str() {
            "mainnet" => "https://mainnet.subfrost.io/v4/api".to_string(),
            "signet" => "https://signet.subfrost.io/v4/api".to_string(),
            "subfrost-regtest" => "https://regtest.subfrost.io/v4/api".to_string(),
            "regtest" | "testnet" | _ => "http://localhost:4000/api/v1".to_string(),
        }
    };
    
    let client = DataApiClient::new(api_url);
    
    match command {
        DataApiCommand::Health { raw_http } => {
            if raw_http {
                let text = client.get_raw("health").await?;
                println!("{}", text);
            } else {
                let result = alkanes_cli_common::dataapi::commands::execute_dataapi_health(&client).await?;
                println!("{}", result);
            }
        }
        DataApiCommand::GetBitcoinPrice { raw, raw_http } => {
            if raw_http {
                let text = client.post_raw("get-bitcoin-price", &json!({})).await?;
                println!("{}", text);
            } else {
                let response = client.get_bitcoin_price().await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_bitcoin_price;
                    print_bitcoin_price(&response.bitcoin);
                }
            }
        }
        DataApiCommand::GetAlkanes { limit, offset, sort_by, order, search, raw, raw_http } => {
            if raw_http {
                let body = json!({
                    "limit": limit,
                    "offset": offset,
                    "sortBy": sort_by,
                    "order": order,
                    "searchQuery": search,
                });
                let text = client.post_raw("get-alkanes", &body).await?;
                println!("{}", text);
            } else {
                let response = client.get_alkanes(limit, offset, sort_by, order, search).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_alkanes_response;
                    print_alkanes_response(&response);
                }
            }
        }
        DataApiCommand::GetAlkanesByAddress { address, raw: _, raw_http } => {
            if raw_http {
                let body = json!({ "address": address });
                let text = client.post_raw("get-alkanes-by-address", &body).await?;
                println!("{}", text);
            } else {
                let tokens = client.get_alkanes_by_address(&address).await?;
                println!("{}", serde_json::to_string_pretty(&tokens)?);
            }
        }
        DataApiCommand::GetAlkaneDetails { id, raw: _, raw_http } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            if raw_http {
                let alkane_id = parse_alkane_id(&id)?;
                let body = json!({ "id": { "block": alkane_id.block.to_string(), "tx": alkane_id.tx.to_string() } });
                let text = client.post_raw("get-alkane-details", &body).await?;
                println!("{}", text);
            } else {
                let alkane_id = parse_alkane_id(&id)?;
                let token = client.get_alkane_details(&alkane_id).await?;
                println!("{}", serde_json::to_string_pretty(&token)?);
            }
        }
        DataApiCommand::GetPools { factory, raw, raw_http } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            if raw_http {
                let factory_id = parse_alkane_id(&factory)?;
                let body = json!({
                    "factoryId": { "block": factory_id.block.to_string(), "tx": factory_id.tx.to_string() }
                });
                let text = client.post_raw("get-pools", &body).await?;
                println!("{}", text);
            } else {
                let factory_id = parse_alkane_id(&factory)?;
                let response = client.get_pools(&factory_id).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_pools_response;
                    print_pools_response(&response.pools);
                }
            }
        }
        DataApiCommand::GetPoolById { id, raw: _, raw_http } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            if raw_http {
                let pool_id = parse_alkane_id(&id)?;
                let body = json!({
                    "poolId": { "block": pool_id.block.to_string(), "tx": pool_id.tx.to_string() }
                });
                let text = client.post_raw("get-pool-by-id", &body).await?;
                println!("{}", text);
            } else {
                let pool_id = parse_alkane_id(&id)?;
                let pool = client.get_pool_by_id(&pool_id).await?;
                println!("{}", serde_json::to_string_pretty(&pool)?);
            }
        }
        DataApiCommand::GetPoolHistory { pool_id, category, limit, offset, raw, raw_http } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            
            let pool_alkane_id = parse_alkane_id(&pool_id)?;
            
            if raw_http {
                let body = json!({
                    "poolId": { "block": pool_alkane_id.block.to_string(), "tx": pool_alkane_id.tx.to_string() },
                    "category": category,
                    "limit": limit,
                    "offset": offset,
                });
                let text = client.post_raw("get-pool-history", &body).await?;
                println!("{}", text);
            } else {
                // get-pool-history now returns same format as get-swap-history (swaps from TraceTrade)
                let history = client.get_swap_history(Some(&pool_alkane_id), limit, offset).await?;
                
                if raw {
                    println!("{}", serde_json::to_string_pretty(&history)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_swap_history;
                    print_swap_history(&history.swaps);
                }
            }
        }
        DataApiCommand::GetSwapHistory { pool_id, limit, offset, raw, raw_http } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            
            let pool_id_str = pool_id.ok_or_else(|| anyhow::anyhow!("--pool-id is required. Specify a pool address like 2:3"))?;
            let pool_alkane_id = parse_alkane_id(&pool_id_str)?;
            
            if raw_http {
                let body = json!({
                    "poolId": { "block": pool_alkane_id.block.to_string(), "tx": pool_alkane_id.tx.to_string() },
                    "limit": limit,
                    "offset": offset,
                });
                let text = client.post_raw("get-swap-history", &body).await?;
                println!("{}", text);
            } else {
                let history = client.get_swap_history(Some(&pool_alkane_id), limit, offset).await?;
                
                if raw {
                    println!("{}", serde_json::to_string_pretty(&history)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_swap_history;
                    print_swap_history(&history.swaps);
                }
            }
        }
        DataApiCommand::GetMarketChart { days, raw, raw_http } => {
            if raw_http {
                let body = json!({ "days": days });
                let text = client.post_raw("get-bitcoin-market-chart", &body).await?;
                println!("{}", text);
            } else {
                let chart = client.get_bitcoin_market_chart(&days).await?;
                if raw {
                    println!("{}", serde_json::to_string_pretty(&chart)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_market_chart;
                    print_market_chart(&chart);
                }
            }
        }
        DataApiCommand::GetHolders { alkane, page, limit, raw, raw_http } => {
            if raw_http {
                let body = json!({
                    "alkane": alkane,
                    "page": page,
                    "limit": limit
                });
                let text = client.post_raw("get-alkane-holders", &body).await?;
                println!("{}", text);
            } else {
                let holders = client.get_holders(&alkane, page, limit).await?;
                if raw {
                    println!("{}", serde_json::to_string(&holders)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_holders_response;
                    print_holders_response(&holders);
                }
            }
        }
        DataApiCommand::GetHolderCount { alkane, raw, raw_http } => {
            if raw_http {
                let body = json!({ "alkane": alkane });
                let text = client.post_raw("get-alkane-holders-count", &body).await?;
                println!("{}", text);
            } else {
                let count = client.get_holders_count(&alkane).await?;
                if raw {
                    println!("{}", serde_json::to_string(&count)?);
                } else {
                    use alkanes_cli_sys::pretty_print::print_holder_count_response;
                    print_holder_count_response(&count);
                }
            }
        }
        DataApiCommand::GetAddressBalances { address, include_outpoints, raw, raw_http } => {
            // Resolve address identifier (e.g., "p2tr:0") to actual address
            let resolved_address = resolve_address_identifier(args, &address).await?;

            if raw_http {
                let body = json!({
                    "address": resolved_address,
                    "include_outpoints": include_outpoints
                });
                let text = client.post_raw("get-address-balances", &body).await?;
                println!("{}", text);
            } else {
                let balances = client.get_address_balances(&resolved_address, include_outpoints).await?;
                if raw {
                    println!("{}", serde_json::to_string(&balances)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&balances)?);
                }
            }
        }
        DataApiCommand::GetOutpointBalances { outpoint, raw, raw_http } => {
            if raw_http {
                let body = json!({ "outpoint": outpoint });
                let text = client.post_raw("get-outpoint-balances", &body).await?;
                println!("{}", text);
            } else {
                let balances = client.get_outpoint_balances(&outpoint).await?;
                if raw {
                    println!("{}", serde_json::to_string(&balances)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&balances)?);
                }
            }
        }
        DataApiCommand::GetBlockHeight { raw, raw_http } => {
            if raw_http {
                let text = client.get_raw("blockheight").await?;
                println!("{}", text);
            } else {
                let result = alkanes_cli_common::dataapi::commands::execute_dataapi_get_block_height(&client).await?;
                if raw {
                    println!("{}", result);
                } else {
                    println!("{}", result);
                }
            }
        }
        DataApiCommand::GetBlockHash { raw, raw_http } => {
            if raw_http {
                let text = client.get_raw("blockhash").await?;
                println!("{}", text);
            } else {
                let result = alkanes_cli_common::dataapi::commands::execute_dataapi_get_block_hash(&client).await?;
                if raw {
                    println!("{}", result);
                } else {
                    println!("{}", result);
                }
            }
        }
        DataApiCommand::GetIndexerPosition { raw, raw_http } => {
            if raw_http {
                let text = client.get_raw("indexer-position").await?;
                println!("{}", text);
            } else {
                let result = alkanes_cli_common::dataapi::commands::execute_dataapi_get_indexer_position(&client).await?;
                if raw {
                    println!("{}", result);
                } else {
                    println!("{}", result);
                }
            }
        }
    }
    Ok(())
}

async fn execute_opi_command(args: &DeezelCommands, command: OpiCommands) -> Result<()> {
    use alkanes_cli_common::opi::{OpiClient, OpiConfig};

    // Determine the OPI URL based on --opi-url flag, jsonrpc-url, or provider network
    let opi_url = if let Some(ref url) = args.opi_url {
        url.clone()
    } else if let Some(ref jsonrpc_url) = args.jsonrpc_url {
        // Derive OPI URL from jsonrpc URL
        // If URL ends with /jsonrpc or /v4/jsonrpc, replace with /opi or /v4/opi
        if jsonrpc_url.ends_with("/jsonrpc") {
            format!("{}/opi", jsonrpc_url.trim_end_matches("/jsonrpc"))
        } else if jsonrpc_url.ends_with("/v4/jsonrpc") {
            format!("{}/v4/opi", jsonrpc_url.trim_end_matches("/v4/jsonrpc"))
        } else {
            // Just append /opi to the base URL
            format!("{}/opi", jsonrpc_url.trim_end_matches('/'))
        }
    } else {
        OpiConfig::default_url_for_network(&args.provider)
    };

    let client = OpiClient::with_headers(opi_url, args.opi_headers.clone());

    match command {
        OpiCommands::BlockHeight => {
            let result = alkanes_cli_common::opi::execute_opi_block_height(&client).await?;
            println!("{}", result);
        }
        OpiCommands::ExtrasBlockHeight => {
            let result = alkanes_cli_common::opi::execute_opi_extras_block_height(&client).await?;
            println!("{}", result);
        }
        OpiCommands::DbVersion => {
            let result = alkanes_cli_common::opi::execute_opi_db_version(&client).await?;
            println!("{}", result);
        }
        OpiCommands::EventHashVersion => {
            let result = alkanes_cli_common::opi::execute_opi_event_hash_version(&client).await?;
            println!("{}", result);
        }
        OpiCommands::BalanceOnBlock { block_height, pkscript, ticker } => {
            let result = alkanes_cli_common::opi::execute_opi_balance_on_block(&client, block_height, &pkscript, &ticker).await?;
            println!("{}", result);
        }
        OpiCommands::ActivityOnBlock { block_height } => {
            let result = alkanes_cli_common::opi::execute_opi_activity_on_block(&client, block_height).await?;
            println!("{}", result);
        }
        OpiCommands::BitcoinRpcResultsOnBlock { block_height } => {
            let result = alkanes_cli_common::opi::execute_opi_bitcoin_rpc_results_on_block(&client, block_height).await?;
            println!("{}", result);
        }
        OpiCommands::CurrentBalance { ticker, address, pkscript } => {
            let result = alkanes_cli_common::opi::execute_opi_current_balance(&client, &ticker, address.as_deref(), pkscript.as_deref()).await?;
            println!("{}", result);
        }
        OpiCommands::ValidTxNotesOfWallet { address, pkscript } => {
            let result = alkanes_cli_common::opi::execute_opi_valid_tx_notes_of_wallet(&client, address.as_deref(), pkscript.as_deref()).await?;
            println!("{}", result);
        }
        OpiCommands::ValidTxNotesOfTicker { ticker } => {
            let result = alkanes_cli_common::opi::execute_opi_valid_tx_notes_of_ticker(&client, &ticker).await?;
            println!("{}", result);
        }
        OpiCommands::Holders { ticker } => {
            let result = alkanes_cli_common::opi::execute_opi_holders(&client, &ticker).await?;
            println!("{}", result);
        }
        OpiCommands::HashOfAllActivity { block_height } => {
            let result = alkanes_cli_common::opi::execute_opi_hash_of_all_activity(&client, block_height).await?;
            println!("{}", result);
        }
        OpiCommands::HashOfAllCurrentBalances => {
            let result = alkanes_cli_common::opi::execute_opi_hash_of_all_current_balances(&client).await?;
            println!("{}", result);
        }
        OpiCommands::Event { inscription_id } => {
            let result = alkanes_cli_common::opi::execute_opi_event(&client, &inscription_id).await?;
            println!("{}", result);
        }
        OpiCommands::Ip => {
            let result = alkanes_cli_common::opi::execute_opi_ip(&client).await?;
            println!("{}", result);
        }
        OpiCommands::Raw { endpoint } => {
            let result = alkanes_cli_common::opi::execute_opi_raw(&client, &endpoint).await?;
            println!("{}", result);
        }

        // ==================== RUNES ====================
        OpiCommands::Runes(runes_cmd) => {
            use crate::commands::OpiRunesCommands;
            match runes_cmd {
                OpiRunesCommands::BlockHeight => {
                    match client.get_runes_block_height().await? {
                        Some(h) => println!("{}", h),
                        None => println!("null"),
                    }
                }
                OpiRunesCommands::BalanceOnBlock { block_height, pkscript, rune_id } => {
                    let result = client.get_runes_balance_on_block(block_height, &pkscript, &rune_id).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::ActivityOnBlock { block_height } => {
                    let result = client.get_runes_activity_on_block(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::CurrentBalance { address, pkscript } => {
                    let result = client.get_runes_current_balance_of_wallet(address.as_deref(), pkscript.as_deref()).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::UnspentOutpoints { address, pkscript } => {
                    let result = client.get_runes_unspent_outpoints_of_wallet(address.as_deref(), pkscript.as_deref()).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::Holders { rune_id } => {
                    let result = client.get_runes_holders(&rune_id).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::HashOfAllActivity { block_height } => {
                    let result = client.get_runes_hash_of_all_activity(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiRunesCommands::Event { txid } => {
                    let result = client.get_runes_event(&txid).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        // ==================== BITMAP ====================
        OpiCommands::Bitmap(bitmap_cmd) => {
            use crate::commands::OpiBitmapCommands;
            match bitmap_cmd {
                OpiBitmapCommands::BlockHeight => {
                    match client.get_bitmap_block_height().await? {
                        Some(h) => println!("{}", h),
                        None => println!("null"),
                    }
                }
                OpiBitmapCommands::HashOfAllActivity { block_height } => {
                    let result = client.get_bitmap_hash_of_all_activity(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiBitmapCommands::HashOfAllBitmaps => {
                    let result = client.get_bitmap_hash_of_all_bitmaps().await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiBitmapCommands::InscriptionId { bitmap } => {
                    let result = client.get_bitmap_inscription_id(&bitmap).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        // ==================== POW20 ====================
        OpiCommands::Pow20(pow20_cmd) => {
            use crate::commands::OpiPow20Commands;
            match pow20_cmd {
                OpiPow20Commands::BlockHeight => {
                    match client.get_pow20_block_height().await? {
                        Some(h) => println!("{}", h),
                        None => println!("null"),
                    }
                }
                OpiPow20Commands::BalanceOnBlock { block_height, pkscript, ticker } => {
                    let result = client.get_pow20_balance_on_block(block_height, &pkscript, &ticker).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::ActivityOnBlock { block_height } => {
                    let result = client.get_pow20_activity_on_block(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::CurrentBalance { ticker, address, pkscript } => {
                    let result = client.get_pow20_current_balance_of_wallet(&ticker, address.as_deref(), pkscript.as_deref()).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::ValidTxNotesOfWallet { address, pkscript } => {
                    let result = client.get_pow20_valid_tx_notes_of_wallet(address.as_deref(), pkscript.as_deref()).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::ValidTxNotesOfTicker { ticker } => {
                    let result = client.get_pow20_valid_tx_notes_of_ticker(&ticker).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::Holders { ticker } => {
                    let result = client.get_pow20_holders(&ticker).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::HashOfAllActivity { block_height } => {
                    let result = client.get_pow20_hash_of_all_activity(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiPow20Commands::HashOfAllCurrentBalances => {
                    let result = client.get_pow20_hash_of_all_current_balances().await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        // ==================== SNS ====================
        OpiCommands::Sns(sns_cmd) => {
            use crate::commands::OpiSnsCommands;
            match sns_cmd {
                OpiSnsCommands::BlockHeight => {
                    match client.get_sns_block_height().await? {
                        Some(h) => println!("{}", h),
                        None => println!("null"),
                    }
                }
                OpiSnsCommands::HashOfAllActivity { block_height } => {
                    let result = client.get_sns_hash_of_all_activity(block_height).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiSnsCommands::HashOfAllRegisteredNames => {
                    let result = client.get_sns_hash_of_all_registered_names().await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiSnsCommands::Info { name } => {
                    let result = client.get_sns_info(&name).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiSnsCommands::InscriptionsOfDomain { domain } => {
                    let result = client.get_sns_inscriptions_of_domain(&domain).await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                OpiSnsCommands::RegisteredNamespaces => {
                    let result = client.get_sns_registered_namespaces().await?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
    }
    Ok(())
}

async fn execute_metashrew_command(provider: &dyn DeezelProvider, command: MetashrewCommands) -> Result<()> {
    match command {
        MetashrewCommands::Height => {
            let height = provider.get_height().await?;
            println!("{height}");
        }
        MetashrewCommands::Getblockhash { height } => {
            let hash = <dyn DeezelProvider as MetashrewProvider>::get_block_hash(provider, height).await?;
            println!("{hash}");
        }
        MetashrewCommands::Getstateroot { height } => {
            let param = match height {
                Some(h) if h.to_lowercase() == "latest" => json!("latest"),
                Some(h) => json!(h.parse::<u64>()?),
                None => json!("latest"),
            };
            let root = alkanes_cli_common::MetashrewProvider::get_state_root(provider, param).await?;
            println!("{root}");
        }
    }
    Ok(())
}

async fn execute_wallet_command<T: System + UtxoProvider>(system: &mut T, command: WalletCommands) -> Result<()> {
    match command {
        WalletCommands::Utxos { addresses, raw, include_frozen } => {
            let resolved_addresses = if let Some(addrs) = addresses {
                let resolved = system.provider().resolve_all_identifiers(&addrs).await?;
                Some(resolved.split(',').map(|s| s.trim().to_string()).collect())
            } else {
                None
            };
            let utxos = system.provider().get_utxos(include_frozen, resolved_addresses).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&utxos)?);
            } else {
                print_utxos(&utxos);
            }
        }
        WalletCommands::Send { address, amount, fee_rate, send_all, from, lock_alkanes, change_address, use_rebar, rebar_tier, auto_confirm } => {
            // Parse BTC amount string and convert to satoshis
            let amount_sats = parse_btc_amount(&amount)?;
            
            let params = alkanes_cli_common::traits::SendParams {
                address,
                amount: amount_sats,
                fee_rate,
                send_all,
                from,
                change_address,
                auto_confirm,
                use_rebar,
                rebar_tier,
                lock_alkanes,
                ordinals_strategy: alkanes_cli_common::alkanes::types::OrdinalsStrategy::default(),
                mempool_indexer: false,
            };
            let txid = system.provider_mut().send(params).await?;
            println!("Transaction sent: {txid}");
        }
        WalletCommands::Balance { addresses, raw } => {
            let resolved_addresses = if let Some(addrs) = addresses {
                let mut resolved = Vec::new();
                for addr in addrs {
                    resolved.push(system.provider().resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };
            let balance = WalletProvider::get_balance(system.provider(), resolved_addresses).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&balance)?);
            } else {
                println!("Confirmed: {}", balance.confirmed);
                println!("Pending:   {}", balance.pending);
            }
        }
        WalletCommands::History { count, address, raw } => {
            let resolved_address = if let Some(addr) = address {
                Some(system.provider().resolve_all_identifiers(&addr).await?)
            } else {
                None
            };
            let history = system.provider().get_history(count, resolved_address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                print_history(&history);
            }
        }
        _ => {
            system.execute_wallet_command(command.into()).await?;
        }
    }
    Ok(())
}

async fn execute_alkanes_command<T: System>(system: &mut T, command: Alkanes) -> Result<()> {
    match command {
        Alkanes::Execute(mut exec_args) => {
            // Resolve any address identifiers before passing them to the executor
            if let Some(change) = &exec_args.change {
                exec_args.change = Some(system.provider().resolve_all_identifiers(change).await?);
            }
            let mut resolved_to = Vec::new();
            for addr in &exec_args.to {
                resolved_to.push(system.provider().resolve_all_identifiers(addr).await?);
            }
            exec_args.to = resolved_to;

            if let Some(from_addrs) = &exec_args.from {
                let mut resolved_from = Vec::new();
                for addr in from_addrs {
                    resolved_from.push(system.provider().resolve_all_identifiers(addr).await?);
                }
                exec_args.from = Some(resolved_from);
            }

            let params = to_enhanced_execute_params(exec_args)?;
            let mut executor = alkanes::execute::EnhancedAlkanesExecutor::new(system.provider_mut());

            // Use execute_full() which implements the presign pattern with atomic broadcasting
            let result = executor.execute_full(params).await?;

            println!("\n✅ Alkanes execution completed successfully!");
            if let Some(commit_txid) = result.commit_txid {
                println!("🔗 Commit TXID: {commit_txid}");
            }
            println!("🔗 Reveal TXID: {}", result.reveal_txid);
            if let Some(commit_fee) = result.commit_fee {
                println!("💰 Commit Fee: {commit_fee} sats");
            }
            println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            if let Some(traces) = result.traces {
                if !traces.is_empty() {
                    println!("\n🔍 Execution Traces:");
                    for (i, trace) in traces.iter().enumerate() {
                        println!("\n📊 Protostone #{} trace:", i + 1);
                        println!("{}", serde_json::to_string_pretty(&trace)?);
                    }
                }
            }
            Ok(())
        },
        Alkanes::Inspect { outpoint, disasm, fuzz, fuzz_ranges, meta, codehash, raw } => {
            let config = alkanes::types::AlkanesInspectConfig {
                disasm,
                fuzz,
                fuzz_ranges,
                meta,
                codehash,
                raw,
            };
            let result = system.provider().inspect(&outpoint, config).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                pretty_print::print_inspection_result(&result);
            }
            Ok(())
        },
        Alkanes::Trace { outpoint, raw } => {
            let result = system.provider().trace(&outpoint).await;
            match result {
                Ok(trace_pb) => {
                    // Convert protobuf trace to alkanes_support::trace::Trace for pretty printing
                    if let Some(alkanes_trace) = trace_pb.trace {
                        // The alkanes_trace is of type alkanes_support::proto::alkanes::AlkanesTrace
                        // which implements Into<alkanes_support::trace::Trace>
                        let trace = alkanes_support::trace::Trace::try_from(
                            prost::Message::encode_to_vec(&alkanes_trace)
                        )?;
                        if raw {
                            let json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                            println!("{}", serde_json::to_string_pretty(&json)?);
                        } else {
                            let pretty = alkanes_cli_common::alkanes::trace::format_trace_pretty(&trace);
                            println!("{}", pretty);
                        }
                    } else {
                        println!("No trace data found for outpoint: {}", outpoint);
                    }
                }
                Err(e) => {
                    println!("Error: {e}");
                }
            }
            Ok(())
        },
        Alkanes::Simulate {
            alkane_id,
            inputs,
            height,
            block,
            transaction,
            envelope,
            pointer,
            txindex,
            refund,
            block_tag,
            raw,
            format
        } => {
            use alkanes_cli_common::proto::alkanes::{MessageContextParcel, AlkaneTransfer, AlkaneId, Uint128};
            use alkanes_cli_common::traits::MetashrewRpcProvider;
            use prost::Message;
            use alkanes_support::envelope::RawEnvelope;
            use bitcoin::{Transaction as BtcTransaction, TxIn, TxOut, OutPoint, Sequence, Amount, Address};
            use bitcoin::transaction::Version;
            
            // Parse alkane_id (format: block:tx:arg1:arg2:..., e.g., 4:20013:2:1717855594)
            let parts: Vec<&str> = alkane_id.split(':').collect();
            if parts.len() < 2 {
                return Err(anyhow::anyhow!("Invalid alkane_id format. Expected block:tx or block:tx:arg1:arg2:..."));
            }

            let target_block: u128 = parts[0].parse()?;
            let target_tx: u128 = parts[1].parse()?;

            // Parse all remaining values as arguments
            let mut cellpack_inputs = Vec::new();
            for i in 2..parts.len() {
                let arg: u128 = parts[i].parse()
                    .map_err(|_| anyhow::anyhow!("Invalid argument at position {}: '{}'", i, parts[i]))?;
                cellpack_inputs.push(arg);
            }
            
            // Parse input alkanes (format: block:tx:amount,block:tx:amount,...)
            let mut alkane_transfers = Vec::new();
            if let Some(inputs_str) = inputs {
                for input in inputs_str.split(',') {
                    let input_parts: Vec<&str> = input.trim().split(':').collect();
                    if input_parts.len() != 3 {
                        return Err(anyhow::anyhow!("Invalid input format '{}'. Expected block:tx:amount", input));
                    }
                    let input_block: u64 = input_parts[0].parse()?;
                    let input_tx: u64 = input_parts[1].parse()?;
                    let input_amount: u128 = input_parts[2].parse()?;
                    
                    alkane_transfers.push(AlkaneTransfer {
                        id: Some(AlkaneId {
                            block: Some(Uint128 {
                                lo: input_block,
                                hi: 0,
                            }),
                            tx: Some(Uint128 {
                                lo: input_tx,
                                hi: 0,
                            }),
                        }),
                        value: Some(Uint128 {
                            lo: (input_amount & 0xFFFFFFFFFFFFFFFF) as u64,
                            hi: (input_amount >> 64) as u64,
                        }),
                    });
                }
            }
            
            // Get height - default to current metashrew_height if not provided
            let simulation_height = if let Some(h) = height {
                h
            } else {
                system.provider().get_metashrew_height().await?
            };
            
            // Parse block hex if provided
            let block_bytes = if let Some(block_hex) = block {
                let hex_str = block_hex.strip_prefix("0x").unwrap_or(&block_hex);
                hex::decode(hex_str)?
            } else {
                Vec::new()
            };
            
            // Parse transaction hex if provided, or create from envelope
            let transaction_bytes = if let Some(tx_hex) = transaction {
                let hex_str = tx_hex.strip_prefix("0x").unwrap_or(&tx_hex);
                hex::decode(hex_str)?
            } else if let Some(envelope_path) = envelope {
                // Read binary file and pack into transaction witness
                let binary_data = std::fs::read(&envelope_path)
                    .map_err(|e| anyhow::anyhow!("Failed to read envelope file '{}': {}", envelope_path, e))?;
                
                log::info!("Read {} bytes from envelope file: {}", binary_data.len(), envelope_path);
                
                // Create envelope and witness
                let raw_envelope = RawEnvelope::from(binary_data);
                let witness = raw_envelope.to_witness(true); // true = compress
                
                // Create a minimal transaction with the witness
                let tx = BtcTransaction {
                    version: Version::ONE,
                    lock_time: bitcoin::absolute::LockTime::ZERO,
                    input: vec![TxIn {
                        previous_output: OutPoint::null(),
                        script_sig: bitcoin::ScriptBuf::new(),
                        sequence: Sequence::MAX,
                        witness,
                    }],
                    output: vec![],
                };
                
                // Serialize the transaction
                use bitcoin::consensus::Encodable;
                let mut tx_bytes = Vec::new();
                tx.consensus_encode(&mut tx_bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to encode transaction: {}", e))?;
                
                log::info!("Created transaction with envelope: {} bytes", tx_bytes.len());
                
                tx_bytes
            } else {
                Vec::new()
            };
            
            // Build calldata using Cellpack for proper encoding
            use alkanes_support::cellpack::Cellpack;
            use alkanes_support::id::AlkaneId as AlkanesAlkaneId;

            let cellpack = Cellpack {
                target: AlkanesAlkaneId {
                    block: target_block,
                    tx: target_tx,
                },
                inputs: cellpack_inputs,
            };
            let calldata = cellpack.encipher();
            
            // Construct MessageContextParcel
            let context = MessageContextParcel {
                alkanes: alkane_transfers.clone(),
                transaction: transaction_bytes.clone(),
                block: block_bytes.clone(),
                height: simulation_height,
                vout: 0,
                txindex,
                calldata: calldata.clone(),
                pointer,
                refund_pointer: refund,
            };
            
            // Debug: Log the context summary
            log::debug!("Simulating alkane {}:{} with {} arguments: {:?}",
                target_block, target_tx, cellpack.inputs.len(), cellpack.inputs);
            log::debug!("Context: height={}, txindex={}, {} input alkanes",
                simulation_height, txindex, context.alkanes.len());
            
            // Run simulation
            let contract_id_str = format!("{}:{}", target_block, target_tx);
            let result = system.provider().simulate(&contract_id_str, &context, block_tag).await?;
            
            // Try to decode the result if it's a hex string
            if let Some(hex_str) = result.as_str() {
                let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
                if let Ok(bytes) = hex::decode(hex_data) {
                    // Try to decode as SimulateResponse
                    use alkanes_cli_common::proto::alkanes::SimulateResponse;
                    if let Ok(sim_response) = SimulateResponse::decode(bytes.as_slice()) {
                        // Check if format is specified
                        if let Some(format_str) = format {
                            use crate::format_parser::OutputFormat;
                            use std::str::FromStr;

                            if let Some(execution) = &sim_response.execution {
                                // Parse format string to enum
                                let output_format = OutputFormat::from_str(&format_str)?;

                                // Parse data according to format
                                match output_format.parse(&execution.data) {
                                    Ok(formatted_json) => {
                                        println!("{}", serde_json::to_string_pretty(&formatted_json)?);
                                        return Ok(());
                                    }
                                    Err(e) => {
                                        // Output error JSON with raw hex
                                        let error_json = json!({
                                            "error": e.to_string(),
                                            "raw_hex": format!("0x{}", hex::encode(&execution.data)),
                                            "byte_count": execution.data.len()
                                        });
                                        println!("{}", serde_json::to_string_pretty(&error_json)?);
                                        return Err(e);
                                    }
                                }
                            } else {
                                return Err(anyhow::anyhow!("No execution data in simulation response"));
                            }
                        }

                        if raw {
                            // Convert SimulateResponse to JSON for raw output
                            let json_response = serde_json::json!({
                                "gas_used": sim_response.gas_used,
                                "error": sim_response.error,
                                "execution": sim_response.execution.as_ref().map(|exec| {
                                    serde_json::json!({
                                        "data": format!("0x{}", hex::encode(&exec.data)),
                                        "alkanes": exec.alkanes.iter().map(|transfer| {
                                            serde_json::json!({
                                                "id": transfer.id.as_ref().map(|id| {
                                                    serde_json::json!({
                                                        "block": id.block.as_ref().map(|b| b.lo).unwrap_or(0),
                                                        "tx": id.tx.as_ref().map(|t| t.lo).unwrap_or(0),
                                                    })
                                                }),
                                                "value": transfer.value.as_ref().map(|v| {
                                                    ((v.hi as u128) << 64) | (v.lo as u128)
                                                }).unwrap_or(0).to_string(),
                                            })
                                        }).collect::<Vec<_>>(),
                                        "storage": exec.storage.iter().map(|kv| {
                                            serde_json::json!({
                                                "key": format!("0x{}", hex::encode(&kv.key)),
                                                "value": format!("0x{}", hex::encode(&kv.value)),
                                            })
                                        }).collect::<Vec<_>>(),
                                    })
                                }),
                            });
                            println!("{}", serde_json::to_string_pretty(&json_response)?);
                        } else {
                            println!("Simulation completed successfully");
                            if let Some(execution) = &sim_response.execution {
                                println!("  Gas used: {}", sim_response.gas_used);
                                println!("  Data: 0x{}", hex::encode(&execution.data));
                                println!("  Alkane transfers: {}", execution.alkanes.len());
                                for (i, transfer) in execution.alkanes.iter().enumerate() {
                                    if let (Some(id), Some(value)) = (&transfer.id, &transfer.value) {
                                        if let (Some(block), Some(tx)) = (&id.block, &id.tx) {
                                            let amount = ((value.hi as u128) << 64) | (value.lo as u128);
                                            println!("    [{}] {}:{} = {}", i, block.lo, tx.lo, amount);
                                        }
                                    }
                                }
                                println!("  Storage changes: {}", execution.storage.len());
                            }
                            if !sim_response.error.is_empty() {
                                println!("  Error: {}", sim_response.error);
                            }
                        }
                        return Ok(());
                    }
                }
            }
            
            // Fallback to raw JSON output
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Simulation result: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        },
        Alkanes::TxScript { envelope, inputs, block_tag, raw } => {
            use alkanes_cli_common::traits::AlkanesProvider;
            use std::fs;
            
            // Read WASM file
            let wasm_bytes = fs::read(&envelope)
                .map_err(|e| anyhow::anyhow!("Failed to read WASM file '{}': {}", envelope, e))?;
            
            println!("Loaded WASM: {} bytes from {}", wasm_bytes.len(), envelope);
            
            // Parse inputs (comma-separated u128 values)
            let mut input_values = Vec::new();
            if let Some(inputs_str) = inputs {
                for input in inputs_str.split(',') {
                    let value: u128 = input.trim().parse()
                        .map_err(|e| anyhow::anyhow!("Invalid input value '{}': {}", input, e))?;
                    input_values.push(value);
                }
                println!("Inputs: {:?}", input_values);
            }
            
            // Execute tx-script
            println!("Executing tx-script...");
            let response_data = system.provider().tx_script(&wasm_bytes, input_values, block_tag).await?;
            
            println!("✅ Got response: {} bytes", response_data.len());
            
            if raw {
                println!("{}", hex::encode(&response_data));
            } else {
                println!("Response (hex): {}", hex::encode(&response_data));
                
                // Try to parse ExtendedCallResponse
                // Format: [alkanes_count(16)][AlkaneTransfers...][storage_count(4)][StorageEntries...][data...]
                if response_data.len() >= 20 {
                    // Read alkanes count (u128 = 16 bytes)
                    let alkanes_count = u128::from_le_bytes(response_data[0..16].try_into()?);
                    println!("  Alkanes count: {}", alkanes_count);
                    
                    // Skip alkanes section (16 + alkanes_count * 48)
                    let alkanes_section_size = 16 + (alkanes_count as usize * 48);
                    
                    if response_data.len() >= alkanes_section_size + 4 {
                        // Read storage count (u32 = 4 bytes)
                        let storage_count = u32::from_le_bytes(response_data[alkanes_section_size..alkanes_section_size+4].try_into()?);
                        println!("  Storage count: {}", storage_count);
                        
                        // For now, assume storage is empty and data starts right after
                        let data_offset = alkanes_section_size + 4;
                        
                        if response_data.len() > data_offset {
                            let data = &response_data[data_offset..];
                            println!("  Data section: {} bytes", data.len());
                            println!("  Data (hex): {}", hex::encode(data));
                        }
                    }
                }
            }
            
            Ok(())
        },
        Alkanes::Sequence { block_tag, raw } => {
            let result = system.provider().sequence(block_tag).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Sequence: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        },
        Alkanes::Spendables { address, raw } => {
            let resolved_address = system.provider().resolve_all_identifiers(&address).await?;
            let result = system.provider().spendables_by_address(&resolved_address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Spendables: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        },
        Alkanes::TraceBlock { height, raw } => {
            let result = system.provider().trace_block(height).await?;
            // The result is AlkanesBlockTraceEvent which contains repeated AlkanesBlockEvent
            if result.events.is_empty() {
                println!("No trace data found for block: {}", height);
            } else {
                let mut all_json_traces = Vec::new();
                let mut all_pretty_traces = Vec::new();

                for (i, block_event) in result.events.iter().enumerate() {
                    if let Some(ref alkanes_trace) = block_event.traces {
                        // Convert via protobuf encoding/decoding
                        match alkanes_support::trace::Trace::try_from(
                            prost::Message::encode_to_vec(alkanes_trace)
                        ) {
                            Ok(trace) => {
                                if raw {
                                    let json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                                    all_json_traces.push(serde_json::json!({
                                        "txindex": block_event.txindex,
                                        "outpoint": block_event.outpoint.as_ref().map(|op| {
                                            let txid_hex = hex::encode(&op.txid);
                                            format!("{}:{}", txid_hex, op.vout)
                                        }),
                                        "trace": json
                                    }));
                                } else {
                                    let outpoint_str = block_event.outpoint.as_ref().map(|op| {
                                        let txid_hex = hex::encode(&op.txid);
                                        format!("{}:{}", txid_hex, op.vout)
                                    }).unwrap_or_else(|| format!("trace #{}", i));
                                    let pretty = alkanes_cli_common::alkanes::trace::format_trace_pretty(&trace);
                                    all_pretty_traces.push(format!("📊 Trace for {} (txindex: {}):\n{}", outpoint_str, block_event.txindex, pretty));
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to decode trace #{}: {}", i, e);
                            }
                        }
                    }
                }

                if raw {
                    println!("{}", serde_json::to_string_pretty(&all_json_traces)?);
                } else {
                    for pretty in all_pretty_traces {
                        println!("{}\n", pretty);
                    }
                }
            }
            Ok(())
        },
        Alkanes::GetBytecode { alkane_id, block_tag, raw } => {
            let result = AlkanesProvider::get_bytecode(system.provider(), &alkane_id, block_tag).await?;
            if raw {
                println!("{result}");
            } else {
                println!("Bytecode: {result}");
            }
            Ok(())
        },
        Alkanes::Meta { alkane_id, block_tag, raw } => {
            let meta_bytes = AlkanesProvider::meta(system.provider(), &alkane_id, block_tag).await?;

            if raw {
                let json_result = serde_json::json!({
                    "alkane_id": alkane_id,
                    "meta": format!("0x{}", hex::encode(&meta_bytes)),
                    "meta_utf8": String::from_utf8_lossy(&meta_bytes).to_string()
                });
                println!("{}", serde_json::to_string_pretty(&json_result)?);
            } else {
                println!("📋 Alkanes Contract Metadata (ABI)");
                println!("═══════════════════════════════════");
                println!("🏷️  Alkane ID: {alkane_id}");

                if meta_bytes.is_empty() {
                    println!("❌ No metadata found for this contract");
                } else {
                    println!("📦 Metadata:");
                    println!("   Length: {} bytes", meta_bytes.len());
                    println!("   Hex: 0x{}", hex::encode(&meta_bytes));

                    // Try to decode as UTF-8 for display
                    if let Ok(meta_str) = String::from_utf8(meta_bytes.clone()) {
                        println!("   UTF-8: {meta_str}");

                        // Try to parse as JSON ABI
                        if let Ok(abi_json) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                            println!("\n🔍 Parsed ABI:");
                            println!("{}", serde_json::to_string_pretty(&abi_json)?);
                        }
                    } else {
                        println!("   (Binary data, not valid UTF-8)");
                    }
                }
            }
            Ok(())
        },
        Alkanes::GetBalance { address, raw } => {
            let resolved_address = if let Some(addr) = &address {
                Some(system.provider().resolve_all_identifiers(addr).await?)
            } else {
                None
            };
            let result = AlkanesProvider::get_balance(system.provider(), resolved_address.as_deref()).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_alkane_balances(&result);
            }
            Ok(())
        }
        Alkanes::WrapBtc { amount, to, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
            use alkanes_cli_common::alkanes::wrap_btc::{WrapBtcExecutor, WrapBtcParams};
            
            let params = WrapBtcParams {
                amount,
                to_address: to.clone(),
                from_addresses: from.clone(),
                change_address: change.clone(),
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
            };

            let mut executor = WrapBtcExecutor::new(system.provider_mut());
            let result = executor.wrap_btc(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ BTC wrapped successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid.as_ref().unwrap_or(&"N/A".to_string()));
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee.unwrap_or(0));
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                println!("🎉 frBTC minted and locked in vault!");
            }
            Ok(())
        }
        Alkanes::Unwrap { block_tag, raw } => {
            let result = system.provider().pending_unwraps(block_tag).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if result.is_empty() {
                    println!("✨ No pending unwraps found");
                    println!();
                    println!("Note: Results are filtered to only show unwraps with spendable UTXOs in your wallet.");
                    println!("      Already fulfilled unwraps are automatically excluded.");
                } else {
                    println!("🔓 Pending Unwraps ({} total):", result.len());
                    println!();
                    println!("Note: Showing only unwraps with spendable UTXOs still available in wallet.");
                    println!("      Already fulfilled unwraps have been filtered out.");
                    println!();
                    
                    let total_amount: u64 = result.iter().map(|u| u.amount).sum();
                    println!("Total unwrap amount: {} sats ({:.8} BTC)", total_amount, total_amount as f64 / 100_000_000.0);
                    println!();
                    
                    for (i, unwrap) in result.iter().enumerate() {
                        println!("  {}. ⏳ Pending", i + 1);
                        println!("     Outpoint: {}:{}", unwrap.txid, unwrap.vout);
                        println!("     Amount:   {} sats ({:.8} BTC)", unwrap.amount, unwrap.amount as f64 / 100_000_000.0);
                        if let Some(ref addr) = unwrap.address {
                            println!("     To:       {}", addr);
                        }
                        println!();
                    }
                }
            }
            Ok(())
        }
        Alkanes::Backtest { txid, raw } => {
            backtest_transaction(system, &txid, raw).await
        }
        Alkanes::GetAllPools { factory, pool_details, experimental_asm, experimental_batch_asm, experimental_asm_parallel, chunk_size, max_concurrent, range, raw } => {
            use alkanes_cli_common::proto::alkanes::{MessageContextParcel, SimulateResponse};
            use alkanes_cli_common::traits::MetashrewRpcProvider;
            use alkanes_cli_common::alkanes::{PoolInfo, PoolDetails};
            use prost::Message;
            
            // Parse factory
            let parts: Vec<&str> = factory.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid factory_id format. Expected 'block:tx'"));
            }
            let factory_block: u64 = parts[0].parse()?;
            let factory_tx: u64 = parts[1].parse()?;
            
            // If experimental_asm is enabled, use the AssemblyScript WASM
            if experimental_asm {
                use alkanes_cli_common::traits::AlkanesProvider;
                
                println!("🚀 Using experimental AssemblyScript WASM...");
                
                // Load get-all-pools WASM
                let wasm_bytes = include_bytes!("../../alkanes-cli-common/src/alkanes/asc/get-all-pools/build/release.wasm");
                
                println!("   Loaded WASM ({} bytes)", wasm_bytes.len());
                println!("   Calling factory {}:{}...", factory_block, factory_tx);
                
                // No inputs needed - the WASM just calls the factory
                let inputs: Vec<u128> = vec![];
                
                // Execute the WASM
                let response_data = system.provider().tx_script(wasm_bytes, inputs, None).await?;
                
                println!("   ✅ Got response: {} bytes", response_data.len());
                println!("   Response (hex): {}", hex::encode(&response_data));
                
                // Try to parse as pool list
                // ExtendedCallResponse format: [alkanes_count(16)][storage_count(4)][pool_count(16)][pools...]
                if response_data.len() >= 36 {
                    let pool_count = u128::from_le_bytes(response_data[20..36].try_into()?);
                    println!("   📊 Pool count: {}", pool_count);
                    
                    let mut offset = 36;
                    let mut pools = Vec::new();
                    while offset + 32 <= response_data.len() {
                        let block = u128::from_le_bytes(response_data[offset..offset+16].try_into()?) as u64;
                        let tx = u128::from_le_bytes(response_data[offset+16..offset+32].try_into()?) as u64;
                        pools.push(format!("{}:{}", block, tx));
                        offset += 32;
                    }
                    
                    println!("   🎯 Parsed {} pools:", pools.len());
                    for (i, pool) in pools.iter().enumerate() {
                        println!("      {}. {}", i + 1, pool);
                    }
                }
                
                return Ok(());
            }
            
            // If experimental_asm_parallel is enabled and pool_details is requested,
            // use parallel WASM-based batch optimization with concurrency control
            if experimental_asm_parallel && pool_details {
                use alkanes_cli_common::traits::AlkanesProvider;
                use futures::stream::{self, StreamExt};
                
                println!("🚀 Using experimental parallel AssemblyScript WASM fetching...");
                
                // Load get-all-pools-details WASM
                let wasm_bytes = include_bytes!("../../alkanes-cli-common/src/alkanes/asc/get-all-pools-details/build/release.wasm");
                
                println!("   Loaded WASM ({} bytes)", wasm_bytes.len());
                
                // First, get total pool count by calling factory
                println!("   Fetching pool list from factory...");
                let get_all_pools_wasm = include_bytes!("../../alkanes-cli-common/src/alkanes/asc/get-all-pools/build/release.wasm");
                let pool_list_data = system.provider().tx_script(get_all_pools_wasm, vec![], None).await?;
                
                // tx_script returns the data field directly: [pool_count(16)][pool0_block(16)][pool0_tx(16)]...
                if pool_list_data.len() < 16 {
                    return Err(anyhow::anyhow!("Invalid pool list response"));
                }
                
                let total_pools = u128::from_le_bytes(pool_list_data[0..16].try_into()?) as usize;
                
                println!("📊 Total pools: {}", total_pools);
                
                // Determine range to fetch
                let (start, end) = if let Some(range_str) = range {
                    let parts: Vec<&str> = range_str.split('-').collect();
                    if parts.len() != 2 {
                        return Err(anyhow::anyhow!("Invalid range format. Use 'start-end' (e.g., '0-50')"));
                    }
                    let start: usize = parts[0].parse()?;
                    let end: usize = parts[1].parse()?;
                    (start, end.min(total_pools - 1))
                } else {
                    (0, total_pools - 1)
                };
                
                let pools_to_fetch = end - start + 1;
                println!("🔄 Fetching pools {} to {} ({} pools) in chunks of {} with max {} concurrent requests...", 
                    start, end, pools_to_fetch, chunk_size, max_concurrent);
                
                // Create chunks
                let mut chunks = Vec::new();
                for chunk_start in (start..=end).step_by(chunk_size) {
                    let chunk_end = (chunk_start + chunk_size - 1).min(end);
                    chunks.push((chunk_start, chunk_end));
                }
                
                println!("   Total chunks: {}", chunks.len());
                
                // Fetch chunks in parallel with concurrency limit
                let provider_arc = std::sync::Arc::new(system.provider().clone_box());
                let results = stream::iter(chunks.into_iter().enumerate())
                    .map(|(idx, (chunk_start, chunk_end))| {
                        let provider = provider_arc.clone();
                        let wasm = wasm_bytes.to_vec();
                        async move {
                            println!("  [{}/...] Fetching chunk {}-{}...", idx + 1, chunk_start, chunk_end);
                            let start_time = std::time::Instant::now();
                            let result = provider.tx_script(
                                &wasm,
                                vec![chunk_start as u128, chunk_end as u128],
                                None,
                            ).await;
                            let elapsed = start_time.elapsed();
                            match &result {
                                Ok(data) => println!("  [{}/...] ✅ Chunk {}-{} complete ({} bytes, {:.2}s)", 
                                    idx + 1, chunk_start, chunk_end, data.len(), elapsed.as_secs_f64()),
                                Err(e) => println!("  [{}/...] ❌ Chunk {}-{} failed: {}", 
                                    idx + 1, chunk_start, chunk_end, e),
                            }
                            (chunk_start, chunk_end, result)
                        }
                    })
                    .buffer_unordered(max_concurrent)
                    .collect::<Vec<_>>()
                    .await;
                
                // Collect and parse results
                println!("\n📦 Parsing results...");
                let mut all_pools: Vec<PoolInfo> = Vec::new();
                
                for (chunk_start, chunk_end, result) in results {
                    let response_data = result?;
                    
                    // tx_script returns the data field directly: [count(16)][pool0_id(32)][size0(8)][data0]...
                    if response_data.len() < 16 {
                        println!("  ⚠️  Chunk {}-{}: Invalid response size", chunk_start, chunk_end);
                        continue;
                    }
                    
                    // response_data IS the pool data
                    let pool_data = &response_data[..];
                    
                    // Parse pool data: [count(16)][pool0_id(32)][size0(8)][data0][pool1_id(32)][size1(8)][data1]...
                    if pool_data.len() < 16 {
                        println!("  ⚠️  Chunk {}-{}: No pool data", chunk_start, chunk_end);
                        continue;
                    }
                    
                    let pool_count_in_chunk = u128::from_le_bytes(pool_data[0..16].try_into()?) as usize;
                    let mut offset = 16;
                    
                    for _ in 0..pool_count_in_chunk {
                        // Read pool ID (32 bytes: 16 for block, 16 for tx)
                        if offset + 32 > pool_data.len() {
                            break;
                        }
                        
                        let pool_block = u128::from_le_bytes(pool_data[offset..offset+16].try_into()?) as u64;
                        let pool_tx = u128::from_le_bytes(pool_data[offset+16..offset+32].try_into()?) as u64;
                        offset += 32;
                        
                        // Read size of this pool's details
                        if offset + 8 > pool_data.len() {
                            break;
                        }
                        let details_size = u64::from_le_bytes(pool_data[offset..offset+8].try_into()?) as usize;
                        offset += 8;
                        
                        if offset + details_size > pool_data.len() {
                            break;
                        }
                        
                        // Parse pool details using the existing PoolDetails::from_bytes
                        let details_bytes = &pool_data[offset..offset+details_size];
                        if let Ok(details) = PoolDetails::from_bytes(details_bytes) {
                            all_pools.push(PoolInfo {
                                pool_id_block: pool_block,
                                pool_id_tx: pool_tx,
                                details: Some(details),
                            });
                        }
                        
                        offset += details_size;
                    }
                }
                
                println!("\n🏊 Successfully fetched {} pool(s) with details", all_pools.len());
                
                // Output results
                if raw {
                    println!("{}", serde_json::to_string_pretty(&all_pools)?);
                } else {
                    for (idx, pool_info) in all_pools.iter().enumerate() {
                        if let Some(details) = &pool_info.details {
                            println!("  {}. Pool {}:{}", idx + 1, pool_info.pool_id_block, pool_info.pool_id_tx);
                            println!("     Name: {}", details.pool_name);
                            println!("     Token A: {}:{} (reserve: {})", details.token_a_block, details.token_a_tx, details.reserve_a);
                            println!("     Token B: {}:{} (reserve: {})", details.token_b_block, details.token_b_tx, details.reserve_b);
                            println!("     LP Supply: {}", details.total_supply);
                            println!();
                        }
                    }
                }
                
                return Ok(());
            }
            
            // If experimental_batch_asm is enabled and pool_details is requested,
            // use WASM-based batch optimization
            if experimental_batch_asm && pool_details {
                use alkanes_cli_common::traits::AlkanesProvider;
                
                println!("🚀 Using experimental AssemblyScript WASM-based batch optimization...");
                
                // Load pre-compiled AssemblyScript WASM
                let wasm_bytes = include_bytes!("../../alkanes-cli-common/src/alkanes/asc/get-pool-details/build/release.wasm");
                
                println!("   Loaded WASM ({} bytes)", wasm_bytes.len());
                
                // First, get total pool count by calling factory directly
                println!("   Fetching pool list from factory...");
                let mut calldata = Vec::new();
                leb128::write::unsigned(&mut calldata, factory_block).unwrap();
                leb128::write::unsigned(&mut calldata, factory_tx).unwrap();
                leb128::write::unsigned(&mut calldata, 3u64).unwrap(); // opcode 3 = GET_ALL_POOLS
                
                let simulation_height = system.provider().get_metashrew_height().await?;
                let context = MessageContextParcel {
                    alkanes: vec![],
                    transaction: vec![],
                    block: vec![],
                    height: simulation_height,
                    vout: 0,
                    txindex: 1,
                    calldata,
                    pointer: 0,
                    refund_pointer: 0,
                };
                
                let factory_id_str = format!("{}:{}", factory_block, factory_tx);
                let result = system.provider().simulate(&factory_id_str, &context, None).await?;
                let hex_str = result.as_str().ok_or_else(|| anyhow::anyhow!("Invalid factory response"))?;
                let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
                let bytes = hex::decode(hex_data)?;
                let sim_response = SimulateResponse::decode(bytes.as_slice())?;
                let factory_data = sim_response.execution.ok_or_else(|| anyhow::anyhow!("No execution in factory response"))?.data;
                
                // Parse pool count from factory response
                let total_pools = {
                    let mut cursor = std::io::Cursor::new(factory_data.clone());
                    use alkanes_cli_common::alkanes::utils::{consume_exact, consume_sized_int};
                    // Skip AlkaneTransferParcel (16 bytes)
                    consume_exact(&mut cursor, 16)?;
                    // Read pool count
                    consume_sized_int::<u128>(&mut cursor)? as usize
                };
                
                println!("📊 Total pools: {}", total_pools);
                
                // Determine range to fetch
                let (start, end) = if let Some(range_str) = range {
                    let parts: Vec<&str> = range_str.split('-').collect();
                    if parts.len() != 2 {
                        return Err(anyhow::anyhow!("Invalid range format. Use 'start-end' (e.g., '0-50')"));
                    }
                    let start: usize = parts[0].parse()?;
                    let end: usize = parts[1].parse()?;
                    (start, end.min(total_pools))
                } else {
                    (0, total_pools)
                };
                
                println!("🔄 Fetching pools {} to {} in chunks of {}...", start, end, chunk_size);
                
                let mut all_pool_data = Vec::new();
                
                // Fetch in chunks
                for chunk_start in (start..end).step_by(chunk_size) {
                    let chunk_end = (chunk_start + chunk_size).min(end);
                    let batch_size = chunk_end - chunk_start;
                    
                    println!("  Fetching chunk {}-{}...", chunk_start, chunk_end - 1);
                    
                    // Call tx_script with [start_index, batch_size]
                    println!("  DEBUG: tx_script inputs = [{}, {}]", chunk_start, batch_size);
                    let response_data = system.provider().tx_script(
                        wasm_bytes,
                        vec![chunk_start as u128, batch_size as u128],
                        Some("latest".to_string()),
                    ).await?;
                    
                    println!("  ✅ Got {} bytes", response_data.len());
                    all_pool_data.push(response_data);
                }
                
                println!("\n📦 Parsing results...");
                let mut all_pools = Vec::new();
                
                for response_data in all_pool_data {
                    // Parse the response
                    let mut cursor = std::io::Cursor::new(response_data.clone());
                    use alkanes_cli_common::alkanes::utils::{consume_exact, consume_sized_int};
                    
                    // Skip alkanes count (16 bytes)
                    consume_exact(&mut cursor, 16)?;
                    
                    // Skip storage count (16 bytes)
                    consume_exact(&mut cursor, 16)?;
                    
                    // Read pool count
                    let pool_count = consume_sized_int::<u128>(&mut cursor)? as usize;
                    
                    println!("  DEBUG: pool_count = {}", pool_count);
                    println!("  DEBUG: cursor position = {}", cursor.position());
                    println!("  DEBUG: response_data length = {}", response_data.len());
                    println!("  DEBUG: First 64 bytes: {}", hex::encode(&response_data[..response_data.len().min(64)]));
                    
                    // Parse each pool
                    for _ in 0..pool_count {
                        // Read pool ID
                        let pool_block = consume_sized_int::<u128>(&mut cursor)? as u64;
                        let pool_tx = consume_sized_int::<u128>(&mut cursor)? as u64;
                        
                        // Read pool details (rest of the data for this pool)
                        // We need to know how many bytes to read - for now use a fixed size
                        // PoolDetails format: token_a(32) + token_b(32) + reserves(48) + name_len(4) + name
                        let details_start = cursor.position() as usize;
                        let remaining_data = &response_data[details_start..];
                        let details = PoolDetails::from_bytes(remaining_data)?;
                        
                        // Advance cursor past the pool details
                        let name_len = remaining_data[112..116].iter()
                            .enumerate()
                            .fold(0u32, |acc, (i, &b)| acc | ((b as u32) << (i * 8)));
                        cursor.set_position((details_start + 116 + name_len as usize) as u64);
                        
                        all_pools.push(PoolInfo {
                            pool_id_block: pool_block,
                            pool_id_tx: pool_tx,
                            details: Some(details),
                        });
                    }
                }
                
                println!("\n🏊 Successfully fetched {} pool(s) with details", all_pools.len());
                
                // Output results
                if raw {
                    println!("{}", serde_json::to_string_pretty(&all_pools)?);
                } else {
                    for pool in &all_pools {
                        println!("\n🏊 Pool {}:{}", pool.pool_id_block, pool.pool_id_tx);
                        if let Some(details) = &pool.details {
                            println!("  Name:      {}", details.pool_name);
                            println!("  Token A:   {}:{} (Reserve: {})", details.token_a_block, details.token_a_tx, details.reserve_a);
                            println!("  Token B:   {}:{} (Reserve: {})", details.token_b_block, details.token_b_tx, details.reserve_b);
                            println!("  LP Supply: {}", details.total_supply);
                            
                            // Calculate price if reserves are non-zero
                            if details.reserve_a > 0 && details.reserve_b > 0 {
                                let price_a_per_b = details.reserve_b as f64 / details.reserve_a as f64;
                                let price_b_per_a = details.reserve_a as f64 / details.reserve_b as f64;
                                println!("  Price A/B: {:.6}", price_a_per_b);
                                println!("  Price B/A: {:.6}", price_b_per_a);
                            }
                        }
                    }
                }
                
                return Ok(());
            }
            
            // Non-batch path: Build calldata for opcode 3 (GET_ALL_POOLS)
            // Build calldata for opcode 3 (GET_ALL_POOLS)
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, factory_block).unwrap();
            leb128::write::unsigned(&mut calldata, factory_tx).unwrap();
            leb128::write::unsigned(&mut calldata, 3u64).unwrap(); // opcode 3
            
            // Get current height
            let simulation_height = system.provider().get_metashrew_height().await?;
            
            // Construct MessageContextParcel
            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: simulation_height,
                vout: 0,
                txindex: 1,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };
            
            // Run simulation
            let contract_id_str = format!("{}:{}", factory_block, factory_tx);
            let result = system.provider().simulate(&contract_id_str, &context, None).await?;
            
            // Parse the result
            if let Some(hex_str) = result.as_str() {
                let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
                if let Ok(bytes) = hex::decode(hex_data) {
                    if let Ok(sim_response) = SimulateResponse::decode(bytes.as_slice()) {
                        if let Some(execution) = &sim_response.execution {
                            // Parse pool list from execution.data
                            // Format: first u128 is count, then pairs of u128s for each pool (block, tx)
                            let data = &execution.data;
                            
                            if data.len() < 16 {
                                return Err(anyhow::anyhow!("Response data too short"));
                            }
                            
                            // Read count (first 16 bytes as u128)
                            let count = u128::from_le_bytes(data[0..16].try_into()?);
                            let mut pools = Vec::new();
                            
                            // Read pool IDs (pairs of u128)
                            let mut offset = 16;
                            for _ in 0..count {
                                if offset + 32 > data.len() {
                                    break;
                                }
                                let pool_block = u128::from_le_bytes(data[offset..offset + 16].try_into()?) as u64;
                                offset += 16;
                                let pool_tx = u128::from_le_bytes(data[offset..offset + 16].try_into()?) as u64;
                                offset += 16;
                                pools.push((pool_block, pool_tx));
                            }
                            
                            // Build Vec<PoolInfo> with optional details
                            let mut pool_infos = Vec::new();
                            
                            for (pool_block, pool_tx) in &pools {
                                let details = if pool_details {
                                    // Fetch details for this pool
                                    let mut pool_calldata = Vec::new();
                                    leb128::write::unsigned(&mut pool_calldata, *pool_block).unwrap();
                                    leb128::write::unsigned(&mut pool_calldata, *pool_tx).unwrap();
                                    leb128::write::unsigned(&mut pool_calldata, 999u64).unwrap();
                                    
                                    let pool_context = MessageContextParcel {
                                        alkanes: vec![],
                                        transaction: vec![],
                                        block: vec![],
                                        height: simulation_height,
                                        vout: 0,
                                        txindex: 1,
                                        calldata: pool_calldata,
                                        pointer: 0,
                                        refund_pointer: 0,
                                    };
                                    
                                    match system.provider().simulate(&format!("{}:{}", pool_block, pool_tx), &pool_context, None).await {
                                        Ok(pool_result) => {
                                            if let Some(pool_hex) = pool_result.as_str() {
                                                let pool_hex_data = pool_hex.strip_prefix("0x").unwrap_or(pool_hex);
                                                if let Ok(pool_bytes) = hex::decode(pool_hex_data) {
                                                    if let Ok(pool_sim) = SimulateResponse::decode(pool_bytes.as_slice()) {
                                                        if let Some(pool_exec) = &pool_sim.execution {
                                                            PoolDetails::from_bytes(&pool_exec.data).ok()
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to fetch details for pool {}:{}: {}", pool_block, pool_tx, e);
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };
                                
                                pool_infos.push(PoolInfo {
                                    pool_id_block: *pool_block,
                                    pool_id_tx: *pool_tx,
                                    details,
                                });
                            }
                            
                            // Output results
                            if raw {
                                println!("{}", serde_json::to_string_pretty(&pool_infos)?);
                            } else {
                                println!("🏊 Found {} pool(s) from factory {}:{}", pool_infos.len(), factory_block, factory_tx);
                                println!();
                                for (idx, pool_info) in pool_infos.iter().enumerate() {
                                    if let Some(details) = &pool_info.details {
                                        println!("  {}. {} ({}:{})", idx + 1, details.pool_name, pool_info.pool_id_block, pool_info.pool_id_tx);
                                        println!("     Token A: {}:{} - Reserve: {}", details.token_a_block, details.token_a_tx, details.reserve_a);
                                        println!("     Token B: {}:{} - Reserve: {}", details.token_b_block, details.token_b_tx, details.reserve_b);
                                        println!("     LP Supply: {}", details.total_supply);
                                        println!();
                                    } else {
                                        println!("  {}. Pool {}:{}", idx + 1, pool_info.pool_id_block, pool_info.pool_id_tx);
                                    }
                                }
                            }
                            
                            return Ok(());
                        }
                    }
                }
            }
            
            // Fallback
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Failed to parse get-all-pools result");
            }
            Ok(())
        }
        Alkanes::AllPoolsDetails { factory_id, raw } => {
            // Parse factory_id to AlkaneId
            let parts: Vec<&str> = factory_id.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid factory_id format. Expected 'block:tx'"));
            }
            let block = parts[0].parse::<u64>()?;
            let tx = parts[1].parse::<u64>()?;
            let factory = alkanes::types::AlkaneId { block, tx };
            
            // Create AMM manager with a temporary executor
            use alkanes::execute::EnhancedAlkanesExecutor;
            let provider = system.provider();
            let mut provider_clone = provider.clone_box();
            let executor = std::sync::Arc::new(EnhancedAlkanesExecutor::new(&mut *provider_clone));
            let amm_manager = alkanes::amm::AmmManager::new(executor);
            
            // Get all pools with details
            let result = amm_manager.get_all_pools_details(&factory, provider).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🏊 Found {} pool(s) with details from factory {}:{}", result.count, block, tx);
                println!();
                for (idx, pool) in result.pools.iter().enumerate() {
                    println!("  {}. Pool {}:{}", idx + 1, pool.pool_id.block, pool.pool_id.tx);
                    println!("     Name: {}", pool.pool_name);
                    println!("     Token0: {}:{} ({})", pool.token0.block, pool.token0.tx, pool.token0_amount);
                    println!("     Token1: {}:{} ({})", pool.token1.block, pool.token1.tx, pool.token1_amount);
                    println!("     LP Supply: {}", pool.token_supply);
                    println!();
                }
            }
            Ok(())
        }
        Alkanes::PoolDetails { pool_id, raw } => {
            use alkanes_cli_common::proto::alkanes::{MessageContextParcel, Uint128};
            use alkanes_cli_common::traits::MetashrewRpcProvider;
            use prost::Message;
            
            // Parse pool_id (format: block:tx)
            let parts: Vec<&str> = pool_id.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid pool_id format. Expected 'block:tx'"));
            }
            let pool_block: u64 = parts[0].parse()?;
            let pool_tx: u64 = parts[1].parse()?;
            
            // Build calldata for opcode 999 (POOL_DETAILS)
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, pool_block).unwrap();
            leb128::write::unsigned(&mut calldata, pool_tx).unwrap();
            leb128::write::unsigned(&mut calldata, 999u64).unwrap(); // opcode 999
            
            // Get current height
            let simulation_height = system.provider().get_metashrew_height().await?;
            
            // Construct MessageContextParcel
            let context = MessageContextParcel {
                alkanes: vec![],
                transaction: vec![],
                block: vec![],
                height: simulation_height,
                vout: 0,
                txindex: 1,
                calldata,
                pointer: 0,
                refund_pointer: 0,
            };
            
            // Run simulation
            let contract_id_str = format!("{}:{}", pool_block, pool_tx);
            let result = system.provider().simulate(&contract_id_str, &context, None).await?;
            
            // Parse the result
            if let Some(hex_str) = result.as_str() {
                let hex_data = hex_str.strip_prefix("0x").unwrap_or(hex_str);
                if let Ok(bytes) = hex::decode(hex_data) {
                    // Try to decode as SimulateResponse
                    use alkanes_cli_common::proto::alkanes::SimulateResponse;
                    if let Ok(sim_response) = SimulateResponse::decode(bytes.as_slice()) {
                        if let Some(execution) = &sim_response.execution {
                            // Parse pool details from execution.data
                            use alkanes_cli_common::alkanes::PoolDetails;
                            match PoolDetails::from_bytes(&execution.data) {
                                Ok(pool_details) => {
                                    if raw {
                                        println!("{}", serde_json::to_string_pretty(&pool_details)?);
                                    } else {
                                        println!("🏊 Pool Details for {}:{}", pool_block, pool_tx);
                                        println!();
                                        println!("  Name:         {}", pool_details.pool_name);
                                        println!("  Token A:      {}:{}", pool_details.token_a_block, pool_details.token_a_tx);
                                        println!("  Reserve A:    {}", pool_details.reserve_a);
                                        println!("  Token B:      {}:{}", pool_details.token_b_block, pool_details.token_b_tx);
                                        println!("  Reserve B:    {}", pool_details.reserve_b);
                                        println!("  LP Supply:    {}", pool_details.total_supply);
                                        
                                        // Calculate and display price ratios
                                        if pool_details.reserve_a > 0 && pool_details.reserve_b > 0 {
                                            let price_a_per_b = pool_details.reserve_b as f64 / pool_details.reserve_a as f64;
                                            let price_b_per_a = pool_details.reserve_a as f64 / pool_details.reserve_b as f64;
                                            println!();
                                            println!("  Price A/B:    {:.6}", price_a_per_b);
                                            println!("  Price B/A:    {:.6}", price_b_per_a);
                                        }
                                    }
                                    return Ok(());
                                }
                                Err(e) => {
                                    return Err(anyhow::anyhow!("Failed to parse pool details: {}", e));
                                }
                            }
                        }
                    }
                }
            }
            
            // Fallback to raw JSON output
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Failed to parse pool details result");
                println!("Raw result: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        }
        Alkanes::ReflectAlkane { alkane_id, concurrency, raw } => {
            use alkanes_cli_common::alkanes::experimental_asm::{reflect_alkane, AlkaneReflection};
            use colored::Colorize;
            
            println!("🔍 Reflecting alkane {} with concurrency {}...", alkane_id, concurrency);
            
            let reflection = reflect_alkane(system.provider(), &alkane_id, concurrency).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&reflection)?);
            } else {
                // Pretty-printed colored output
                println!("\n📊 Alkane Metadata for {}", alkane_id.bright_cyan().bold());
                println!("{}", "═".repeat(60).bright_black());
                
                if let Some(name) = &reflection.name {
                    println!("  {} {}", "Name:".bright_yellow(), name.bright_white());
                }
                
                if let Some(symbol) = &reflection.symbol {
                    println!("  {} {}", "Symbol:".bright_yellow(), symbol.bright_white());
                }
                
                if let Some(total_supply) = reflection.total_supply {
                    println!("  {} {}", "Total Supply:".bright_yellow(), total_supply.to_string().bright_white());
                }
                
                if let Some(cap) = reflection.cap {
                    println!("  {} {}", "Cap:".bright_yellow(), cap.to_string().bright_white());
                }
                
                if let Some(minted) = reflection.minted {
                    println!("  {} {}", "Minted:".bright_yellow(), minted.to_string().bright_white());
                    
                    if let Some(cap_val) = reflection.cap {
                        if cap_val > 0 {
                            let progress = (minted as f64 / cap_val as f64) * 100.0;
                            let bar_length = 40;
                            let filled = ((progress / 100.0) * bar_length as f64) as usize;
                            let bar = format!(
                                "[{}{}] {:.1}%",
                                "█".repeat(filled).bright_green(),
                                "░".repeat(bar_length - filled).bright_black(),
                                progress
                            );
                            println!("  {} {}", "Progress:".bright_yellow(), bar);
                        }
                    }
                }
                
                if let Some(value_per_mint) = reflection.value_per_mint {
                    println!("  {} {}", "Value Per Mint:".bright_yellow(), value_per_mint.to_string().bright_white());
                }
                
                if let Some(data) = &reflection.data {
                    println!("  {} 0x{}", "Data:".bright_yellow(), data.bright_white());
                }
                
                println!("{}", "═".repeat(60).bright_black());
            }
            
            Ok(())
        }
        Alkanes::ReflectAlkaneRange { block, start_tx, end_tx, concurrency, raw } => {
            use alkanes_cli_common::alkanes::experimental_asm::reflect_alkane_range;
            use colored::Colorize;
            
            let count = end_tx - start_tx + 1;
            println!("🔍 Reflecting {} alkanes ({}:{} to {}:{}) with concurrency {}...", 
                     count, block, start_tx, block, end_tx, concurrency);
            
            let reflections = reflect_alkane_range(system.provider(), block, start_tx, end_tx, concurrency).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&reflections)?);
            } else {
                println!("\n✅ Successfully reflected {} alkanes\n", reflections.len());
                
                for (idx, reflection) in reflections.iter().enumerate() {
                    println!("{} {}", 
                             format!("{}.", idx + 1).bright_black(),
                             reflection.id.bright_cyan().bold());
                    
                    if let Some(name) = &reflection.name {
                        print!("   {} ", name.bright_white());
                    }
                    if let Some(symbol) = &reflection.symbol {
                        print!("({})", symbol.bright_yellow());
                    }
                    println!();
                    
                    if let Some(total_supply) = reflection.total_supply {
                        println!("   Supply: {}", total_supply.to_string().bright_white());
                    }
                    
                    if let Some(minted) = reflection.minted {
                        if let Some(cap) = reflection.cap {
                            if cap > 0 {
                                let progress = (minted as f64 / cap as f64) * 100.0;
                                println!("   Minted: {} / {} ({:.1}%)", 
                                         minted.to_string().bright_green(),
                                         cap.to_string().bright_white(),
                                         progress);
                            }
                        }
                    }
                    
                    println!();
                }
                
                println!("{}", "═".repeat(60).bright_black());
                println!("📊 Total: {} alkanes reflected", reflections.len().to_string().bright_green().bold());
            }
            
            Ok(())
        }
        Alkanes::InitPool { pair, liquidity, to, from, change, minimum, fee_rate, trace, factory, auto_confirm } => {
            use alkanes_cli_common::alkanes::amm_cli::{init_pool, InitPoolParams};
            use alkanes_cli_common::alkanes::types::AlkaneId;
            
            // Parse pair (e.g., "2:0,32:0")
            let pair_parts: Vec<&str> = pair.split(',').collect();
            if pair_parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid pair format. Expected BLOCK:TX,BLOCK:TX"));
            }
            
            let parse_id = |s: &str| -> anyhow::Result<AlkaneId> {
                let parts: Vec<&str> = s.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!("Invalid ID format"));
                }
                Ok(AlkaneId {
                    block: parts[0].parse()?,
                    tx: parts[1].parse()?,
                })
            };
            
            let token0 = parse_id(pair_parts[0])?;
            let token1 = parse_id(pair_parts[1])?;
            let factory_id = parse_id(&factory)?;
            
            // Parse liquidity amounts (e.g., "300000000:50000")
            let liq_parts: Vec<&str> = liquidity.split(':').collect();
            if liq_parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid liquidity format. Expected AMOUNT0:AMOUNT1"));
            }
            
            let amount0: u128 = liq_parts[0].parse()?;
            let amount1: u128 = liq_parts[1].parse()?;
            
            let params = InitPoolParams {
                factory_id,
                token0,
                token1,
                amount0,
                amount1,
                minimum_lp: minimum.clone(),
                to_address: to.clone(),
                from_address: from.clone(),
                change_address: change.clone(),
                fee_rate: fee_rate.clone(),
                trace: trace.clone(),
                auto_confirm: auto_confirm.clone(),
            };
            
            let provider = system.provider_mut();
            let txid = init_pool(provider, params).await?;
            println!("Transaction ID: {}", txid);
            Ok(())
        }
        Alkanes::Swap { 
            path, 
            input, 
            minimum_output, 
            slippage, 
            expires, 
            to, 
            from, 
            change, 
            fee_rate, 
            trace, 
            mine,
            factory,
            no_optimize,
            auto_confirm,
        } => {
            use alkanes_cli_common::proto::alkanes::{MessageContextParcel, SimulateResponse};
            use alkanes_cli_common::traits::MetashrewRpcProvider;
            use alkanes_cli_common::alkanes::{PoolInfo, PoolDetails};
            use alkanes_cli_common::alkanes::types::AlkaneId;
            use prost::Message;
            use std::io::{self, Write};
            
            // Parse path - comma-separated alkane IDs (e.g., "2:0,32:0" or "B,2:0,32:0")
            // "B" represents Bitcoin and triggers wrap (at start) or unwrap (at end)
            let path_parts: Vec<&str> = path.split(',').collect();
            if path_parts.len() < 2 {
                return Err(anyhow::anyhow!("Swap path must have at least 2 tokens"));
            }
            
            // Check for wrap (B at start) and unwrap (B at end)
            let needs_wrap = path_parts[0].to_uppercase() == "B";
            let needs_unwrap = path_parts[path_parts.len() - 1].to_uppercase() == "B";
            
            // Validate B only at start/end, not in middle
            for (i, part) in path_parts.iter().enumerate() {
                if part.to_uppercase() == "B" {
                    if i != 0 && i != path_parts.len() - 1 {
                        return Err(anyhow::anyhow!("'B' can only appear at the start or end of the path, not in the middle"));
                    }
                }
            }
            
            // Parse path tokens, replacing "B" with "32:0" (frBTC)
            let path_tokens: Result<Vec<AlkaneId>, _> = path_parts
                .iter()
                .map(|token_str| {
                    if token_str.to_uppercase() == "B" {
                        // B represents frBTC (32:0)
                        return Ok(AlkaneId { block: 32, tx: 0 });
                    }
                    let parts: Vec<&str> = token_str.split(':').collect();
                    if parts.len() != 2 {
                        return Err(anyhow::anyhow!("Invalid alkane ID format: {}. Expected BLOCK:TX or 'B'", token_str));
                    }
                    Ok(AlkaneId {
                        block: parts[0].parse()?,
                        tx: parts[1].parse()?,
                    })
                })
                .collect();
            let mut path_tokens = path_tokens?;
            
            let input_token = path_tokens[0].clone();
            let output_token = path_tokens[path_tokens.len() - 1].clone();
            
            // Show wrap/unwrap status
            if needs_wrap {
                println!("🔧 Wrapping BTC → frBTC ({}:{})", input_token.block, input_token.tx);
            }
            if needs_unwrap {
                println!("🔓 Unwrapping frBTC ({}:{}) → BTC", output_token.block, output_token.tx);
            }
            
            println!("🔄 Preparing swap: {}:{} → {}:{}", 
                     input_token.block, input_token.tx,
                     output_token.block, output_token.tx);
            println!("💰 Input amount: {}", input);
            
            // Get current height
            let current_height = system.provider().get_height().await?;
            let expires_block = expires.unwrap_or(current_height + 100);
            println!("⏰ Expires at block: {}", expires_block);
            
            // Parse factory
            let factory_parts: Vec<&str> = factory.split(':').collect();
            if factory_parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid factory format. Expected 'block:tx'"));
            }
            let factory_block: u64 = factory_parts[0].parse()?;
            let factory_tx: u64 = factory_parts[1].parse()?;
            
            // Path optimization: Get all pools and find optimal route
            let optimal_path = if !no_optimize {
                println!("\n🔍 Analyzing pool liquidity for optimal path...");
                
                // Get all pools with details
                let mut pool_calldata = Vec::new();
                leb128::write::unsigned(&mut pool_calldata, factory_block).unwrap();
                leb128::write::unsigned(&mut pool_calldata, factory_tx).unwrap();
                leb128::write::unsigned(&mut pool_calldata, 3u64).unwrap(); // GET_ALL_POOLS opcode
                
                let context = MessageContextParcel {
                    alkanes: vec![],
                    transaction: vec![],
                    block: vec![],
                    height: current_height,
                    vout: 0,
                    txindex: 1,
                    calldata: pool_calldata,
                    pointer: 0,
                    refund_pointer: 0,
                };
                
                let result = system.provider().simulate(&factory, &context, None).await?;
                let hex_data = result.as_str().ok_or_else(|| anyhow::anyhow!("Expected string result"))?;
                let hex_data = hex_data.strip_prefix("0x").unwrap_or(hex_data);
                let bytes = hex::decode(hex_data)?;
                let sim_response = SimulateResponse::decode(bytes.as_slice())?;
                
                let pools = if let Some(execution) = &sim_response.execution {
                    let data = &execution.data;
                    if data.len() < 16 {
                        return Err(anyhow::anyhow!("Invalid response from factory"));
                    }
                    
                    let count = u128::from_le_bytes(data[0..16].try_into().unwrap()) as usize;
                    let mut pools = Vec::new();
                    
                    for i in 0..count {
                        let offset = 16 + (i * 32);
                        if offset + 32 > data.len() {
                            break;
                        }
                        let pool_block = u128::from_le_bytes(data[offset..offset+16].try_into().unwrap()) as u64;
                        let pool_tx = u128::from_le_bytes(data[offset+16..offset+32].try_into().unwrap()) as u64;
                        pools.push((pool_block, pool_tx));
                    }
                    pools
                } else {
                    Vec::new()
                };
                
                // Fetch details for all pools
                let mut pool_infos = Vec::new();
                for (pool_block, pool_tx) in &pools {
                    let mut detail_calldata = Vec::new();
                    leb128::write::unsigned(&mut detail_calldata, *pool_block).unwrap();
                    leb128::write::unsigned(&mut detail_calldata, *pool_tx).unwrap();
                    leb128::write::unsigned(&mut detail_calldata, 999u64).unwrap();
                    
                    let detail_context = MessageContextParcel {
                        alkanes: vec![],
                        transaction: vec![],
                        block: vec![],
                        height: current_height,
                        vout: 0,
                        txindex: 1,
                        calldata: detail_calldata,
                        pointer: 0,
                        refund_pointer: 0,
                    };
                    
                    if let Ok(detail_result) = system.provider().simulate(&format!("{}:{}", pool_block, pool_tx), &detail_context, None).await {
                        if let Some(detail_hex) = detail_result.as_str() {
                            let detail_hex = detail_hex.strip_prefix("0x").unwrap_or(detail_hex);
                            if let Ok(detail_bytes) = hex::decode(detail_hex) {
                                if let Ok(detail_sim) = SimulateResponse::decode(detail_bytes.as_slice()) {
                                    if let Some(detail_exec) = &detail_sim.execution {
                                        if let Ok(details) = PoolDetails::from_bytes(&detail_exec.data) {
                                            pool_infos.push(PoolInfo {
                                                pool_id_block: *pool_block,
                                                pool_id_tx: *pool_tx,
                                                details: Some(details),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("   Found {} pools with liquidity data", pool_infos.len());
                
                // Filter pools by minimum liquidity threshold
                // Only consider pools with at least 1000 units of liquidity in both reserves
                const MIN_LIQUIDITY: u128 = 1000;
                let liquid_pools: Vec<_> = pool_infos.iter()
                    .filter(|p| {
                        if let Some(details) = &p.details {
                            details.reserve_a >= MIN_LIQUIDITY && details.reserve_b >= MIN_LIQUIDITY
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();
                
                println!("   {} pools have sufficient liquidity", liquid_pools.len());
                
                // Build a map of all tokens in liquid pools for faster lookup
                let mut available_tokens = std::collections::HashSet::new();
                for pool in &liquid_pools {
                    if let Some(details) = &pool.details {
                        available_tokens.insert(AlkaneId { block: details.token_a_block, tx: details.token_a_tx });
                        available_tokens.insert(AlkaneId { block: details.token_b_block, tx: details.token_b_tx });
                    }
                }
                
                // Calculate output for user's path (if it's viable)
                let user_path_output = calculate_path_output(&path_tokens, &liquid_pools, input)
                    .unwrap_or(0);
                
                // Try to find a better path (1-4 hops)
                let mut best_path = path_tokens.clone();
                let mut best_output = user_path_output;
                
                // 1-hop: Check direct swap
                if let Some(direct_output) = find_direct_pool(&input_token, &output_token, &liquid_pools, input) {
                    if direct_output > best_output {
                        best_path = vec![input_token.clone(), output_token.clone()];
                        best_output = direct_output;
                    }
                }
                
                // 2-hop: input -> intermediate -> output
                for pool in &liquid_pools {
                    if let Some(details) = &pool.details {
                        let intermediate_a = AlkaneId { block: details.token_a_block, tx: details.token_a_tx };
                        let intermediate_b = AlkaneId { block: details.token_b_block, tx: details.token_b_tx };
                        
                        for intermediate in [intermediate_a, intermediate_b] {
                            if intermediate != input_token && intermediate != output_token {
                                let candidate_path = vec![input_token.clone(), intermediate.clone(), output_token.clone()];
                                if let Ok(output) = calculate_path_output(&candidate_path, &liquid_pools, input) {
                                    if output > best_output {
                                        best_path = candidate_path;
                                        best_output = output;
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 3-hop: input -> mid1 -> mid2 -> output
                // Only check 3-hop if we have enough pools and tokens
                if liquid_pools.len() >= 3 {
                    let mut checked_paths = std::collections::HashSet::new();
                    for pool1 in &liquid_pools {
                        if let Some(details1) = &pool1.details {
                            for mid1 in [
                                AlkaneId { block: details1.token_a_block, tx: details1.token_a_tx },
                                AlkaneId { block: details1.token_b_block, tx: details1.token_b_tx }
                            ] {
                                if mid1 == input_token || mid1 == output_token {
                                    continue;
                                }
                                
                                for pool2 in &liquid_pools {
                                    if pool2.pool_id_block == pool1.pool_id_block && pool2.pool_id_tx == pool1.pool_id_tx {
                                        continue;
                                    }
                                    
                                    if let Some(details2) = &pool2.details {
                                        for mid2 in [
                                            AlkaneId { block: details2.token_a_block, tx: details2.token_a_tx },
                                            AlkaneId { block: details2.token_b_block, tx: details2.token_b_tx }
                                        ] {
                                            if mid2 == input_token || mid2 == output_token || mid2 == mid1 {
                                                continue;
                                            }
                                            
                                            let path_key = format!("{}:{}:{}", mid1.block, mid1.tx, mid2.block);
                                            if checked_paths.contains(&path_key) {
                                                continue;
                                            }
                                            checked_paths.insert(path_key);
                                            
                                            let candidate_path = vec![
                                                input_token.clone(),
                                                mid1.clone(),
                                                mid2.clone(),
                                                output_token.clone()
                                            ];
                                            
                                            if let Ok(output) = calculate_path_output(&candidate_path, &liquid_pools, input) {
                                                if output > best_output {
                                                    best_path = candidate_path;
                                                    best_output = output;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 4-hop: input -> mid1 -> mid2 -> mid3 -> output
                // Only check if we have many pools (>= 4) and best output is still not great
                if liquid_pools.len() >= 4 && best_output < input * 8 / 10 {
                    // Only explore 4-hop if 3-hop didn't find a good path
                    // Limit search space to most liquid pools
                    let mut sorted_pools = liquid_pools.clone();
                    sorted_pools.sort_by(|a, b| {
                        let a_liq = a.details.as_ref().map(|d| d.reserve_a + d.reserve_b).unwrap_or(0);
                        let b_liq = b.details.as_ref().map(|d| d.reserve_a + d.reserve_b).unwrap_or(0);
                        b_liq.cmp(&a_liq)
                    });
                    let top_pools: Vec<_> = sorted_pools.iter().take(10.min(sorted_pools.len())).cloned().collect();
                    
                    let mut checked_paths = std::collections::HashSet::new();
                    for pool1 in &top_pools {
                        if let Some(details1) = &pool1.details {
                            for mid1 in [
                                AlkaneId { block: details1.token_a_block, tx: details1.token_a_tx },
                                AlkaneId { block: details1.token_b_block, tx: details1.token_b_tx }
                            ] {
                                if mid1 == input_token || mid1 == output_token {
                                    continue;
                                }
                                
                                for pool2 in &top_pools {
                                    if let Some(details2) = &pool2.details {
                                        for mid2 in [
                                            AlkaneId { block: details2.token_a_block, tx: details2.token_a_tx },
                                            AlkaneId { block: details2.token_b_block, tx: details2.token_b_tx }
                                        ] {
                                            if mid2 == input_token || mid2 == output_token || mid2 == mid1 {
                                                continue;
                                            }
                                            
                                            for pool3 in &top_pools {
                                                if let Some(details3) = &pool3.details {
                                                    for mid3 in [
                                                        AlkaneId { block: details3.token_a_block, tx: details3.token_a_tx },
                                                        AlkaneId { block: details3.token_b_block, tx: details3.token_b_tx }
                                                    ] {
                                                        if mid3 == input_token || mid3 == output_token || mid3 == mid1 || mid3 == mid2 {
                                                            continue;
                                                        }
                                                        
                                                        let path_key = format!("{}:{}:{}:{}", mid1.block, mid1.tx, mid2.block, mid3.block);
                                                        if checked_paths.contains(&path_key) {
                                                            continue;
                                                        }
                                                        checked_paths.insert(path_key);
                                                        
                                                        let candidate_path = vec![
                                                            input_token.clone(),
                                                            mid1.clone(),
                                                            mid2.clone(),
                                                            mid3.clone(),
                                                            output_token.clone()
                                                        ];
                                                        
                                                        if let Ok(output) = calculate_path_output(&candidate_path, &liquid_pools, input) {
                                                            if output > best_output {
                                                                best_path = candidate_path;
                                                                best_output = output;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // If we found a better path, prompt user
                if best_path != path_tokens && best_output > user_path_output {
                    let improvement = ((best_output as f64 - user_path_output as f64) / user_path_output as f64) * 100.0;
                    println!("\n✨ Found a better path with {:.2}% more output!", improvement);
                    println!("   User path: {} → expected output: {}", 
                             format_path(&path_tokens), user_path_output);
                    println!("   Optimal path: {} → expected output: {}", 
                             format_path(&best_path), best_output);
                    
                    if !auto_confirm {
                        print!("\n   [Y] Use optimal path  [C] Continue with your path  [N] Cancel\n   Choice: ");
                        io::stdout().flush()?;
                        
                        let mut choice = String::new();
                        io::stdin().read_line(&mut choice)?;
                        let choice = choice.trim().to_uppercase();
                        
                        match choice.as_str() {
                            "Y" => {
                                println!("   Using optimal path");
                                Some(best_path)
                            }
                            "C" => {
                                println!("   Continuing with your path");
                                None
                            }
                            "N" | _ => {
                                println!("   Swap cancelled");
                                return Ok(());
                            }
                        }
                    } else {
                        println!("   Auto-confirm: using optimal path");
                        Some(best_path)
                    }
                } else {
                    println!("   Your path is already optimal!");
                    None
                }
            } else {
                println!("\n⏩ Path optimization skipped");
                None
            };
            
            // Use optimal path if found, otherwise use user's path
            if let Some(opt_path) = optimal_path {
                path_tokens = opt_path;
            }
            
            println!("\n🎯 Final path: {}", format_path(&path_tokens));
            println!("   Path length: {} hops", path_tokens.len() - 1);
            
            // Calculate expected output using pool reserves
            println!("\n📊 Calculating expected output using pool liquidity...");
            
            // Get pool details for all hops in the path
            let mut pool_infos = Vec::new();
            for i in 0..path_tokens.len() - 1 {
                let pool_id = find_pool_for_pair(&path_tokens[i], &path_tokens[i + 1], system, &factory, current_height).await?;
                if let Ok(details) = get_pool_details(system, pool_id.0, pool_id.1, current_height).await {
                    pool_infos.push(alkanes_cli_common::alkanes::PoolInfo {
                        pool_id_block: pool_id.0,
                        pool_id_tx: pool_id.1,
                        details: Some(details),
                    });
                } else {
                    return Err(anyhow::anyhow!("Could not fetch details for pool {}:{}", pool_id.0, pool_id.1));
                }
            }
            
            // Calculate expected output using constant product formula
            let expected_output = calculate_path_output(&path_tokens, &pool_infos, input)?;
            
            println!("   Expected output: {} tokens", expected_output);
            
            // Calculate minimum output with slippage
            let final_minimum_output = if let Some(min_out) = minimum_output {
                println!("   Using explicit minimum output: {}", min_out);
                min_out
            } else {
                let slippage_factor = 1.0 - (slippage / 100.0);
                let min_out = (expected_output as f64 * slippage_factor) as u128;
                println!("   Applying {}% slippage: minimum output = {}", slippage, min_out);
                min_out
            };
            
            // Build protostone for swap
            // Always use factory routing (opcode 13 = swap_exact_tokens_for_tokens)
            // This works for both single-hop and multi-hop swaps
            println!("\n🔨 Building swap transaction...");
            
            println!("   Using factory routing");
            println!("   Path: {}", format_path(&path_tokens));
            println!("   Input: {} of {}:{}", input, input_token.block, input_token.tx);
            println!("   Minimum output: {} of {}:{}", final_minimum_output, output_token.block, output_token.tx);
            
            // Format: [factoryBlock,factoryTx,13,path_length,token0_block,token0_tx,...,amount_in,min_out,deadline]
            let mut inputs = vec![
                factory_block.to_string(),
                factory_tx.to_string(),
                "13".to_string(), // opcode for swap_exact_tokens_for_tokens
                path_tokens.len().to_string(),
            ];
            
            // Add flattened path (block, tx pairs)
            for token in &path_tokens {
                inputs.push(token.block.to_string());
                inputs.push(token.tx.to_string());
            }
            
            // Add amount_in, min_out, deadline
            inputs.push(input.to_string());
            inputs.push(final_minimum_output.to_string());
            inputs.push(expires_block.to_string());
            
            let calldata = format!("[{}]", inputs.join(","));
            
            println!("   Calldata: {}", calldata);
            
            // Fetch subfrost address if wrap or unwrap is needed
            let subfrost_address = if needs_wrap || needs_unwrap {
                use alkanes_cli_common::subfrost::get_subfrost_address;
                use alkanes_cli_common::alkanes::types::AlkaneId as CliAlkaneId;
                let frbtc_id = CliAlkaneId { block: 32, tx: 0 };
                let addr = get_subfrost_address(system.provider(), &frbtc_id).await?;
                println!("📍 Subfrost address: {}", addr);
                Some(addr)
            } else {
                None
            };
            
            // Use existing execute logic
            use alkanes_cli_common::alkanes::parsing::parse_protostones;
            use alkanes_cli_common::alkanes::execute::{EnhancedAlkanesExecutor, EnhancedExecuteParams};
            use alkanes_cli_common::alkanes::types::{InputRequirement, OutputTarget};
            
            // Build to_addresses based on wrap/unwrap needs
            let to_addresses = if needs_wrap && needs_unwrap {
                // Both wrap and unwrap: subfrost(wrap), intermediate, btc_recipient, subfrost(unwrap)
                vec![
                    subfrost_address.clone().unwrap(),  // Output 0: BTC payment for wrap
                    to.clone(),                          // Output 1: Intermediate recipient
                    to.clone(),                          // Output 2: BTC recipient (unwrap destination)
                    subfrost_address.clone().unwrap(),  // Output 3: Dust for unwrap cellpack
                ]
            } else if needs_wrap {
                // Wrap only: subfrost(wrap), recipient
                vec![
                    subfrost_address.clone().unwrap(),  // Output 0: BTC payment for wrap
                    to.clone(),                          // Output 1: frBTC recipient
                ]
            } else if needs_unwrap {
                // Unwrap only: recipient, btc_recipient, subfrost(unwrap)
                vec![
                    to.clone(),                          // Output 0: Alkanes recipient
                    to.clone(),                          // Output 1: BTC recipient (unwrap destination)
                    subfrost_address.clone().unwrap(),  // Output 2: Dust for unwrap cellpack
                ]
            } else {
                // Normal swap: just recipient
                vec![to.clone()]
            };
            
            // Build input requirements based on wrap needs
            let input_reqs = if needs_wrap {
                // Wrap requires BTC input instead of alkanes input
                vec![InputRequirement::Bitcoin { amount: input as u64 }]
            } else {
                // Normal swap or unwrap: alkanes input
                vec![InputRequirement::Alkanes {
                    block: input_token.block,
                    tx: input_token.tx,
                    amount: input as u64,
                }]
            };
            
            // Parse the swap protostone from calldata
            let mut protostones = parse_protostones(&calldata)?;
            
            // Prepend wrap protostone if needed
            if needs_wrap {
                println!("🔧 Adding wrap protostone: BTC → frBTC");
                // Wrap outputs frBTC to output 1 (recipient)
                let wrap_proto = build_wrap_protostone(input as u64, 1);
                protostones.insert(0, wrap_proto);
            }
            
            // Append unwrap protostone if needed
            if needs_unwrap {
                println!("🔓 Adding unwrap protostone: frBTC → BTC");
                
                // Calculate output indices based on to_addresses structure
                let (btc_recipient_vout, subfrost_dust_vout, refund_vout) = if needs_wrap && needs_unwrap {
                    (2u32, 3u32, 1u32)  // wrap+unwrap: btc=2, dust=3, refund=1
                } else {
                    (1u32, 2u32, 0u32)  // unwrap only: btc=1, dust=2, refund=0
                };
                
                // Modify the last protostone (swap) to point to the unwrap protostone
                // The unwrap will be at protostone index = protostones.len()
                let unwrap_protostone_index = protostones.len() as u32;
                
                if let Some(swap_proto) = protostones.last_mut() {
                    println!("   ↪ Swap protostone pointing to unwrap protostone #{}", unwrap_protostone_index);
                    swap_proto.pointer = Some(OutputTarget::Protostone(unwrap_protostone_index));
                }
                
                // Build and append unwrap protostone
                let unwrap_proto = build_unwrap_protostone(
                    final_minimum_output,    // Amount of frBTC to unwrap
                    subfrost_dust_vout,      // Dust output index for cellpack
                    btc_recipient_vout,      // Where unwrapped BTC goes
                    refund_vout,             // Refund if unwrap fails
                );
                protostones.push(unwrap_proto);
            }
            
            let mut executor = EnhancedAlkanesExecutor::new(system.provider_mut());
            let execute_params = EnhancedExecuteParams {
                input_requirements: input_reqs,
                alkanes_change_address: Some(from.clone()),
                to_addresses,
                from_addresses: Some(vec![from.clone()]),
                change_address: change.clone(),
                fee_rate: fee_rate.map(|f| f as f32),
                envelope_data: None,
                protostones,
                raw_output: false,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
                ordinals_strategy: alkanes_cli_common::alkanes::types::OrdinalsStrategy::default(),
                mempool_indexer: false,
            };

            println!("\n📤 Executing swap...");
            let state = executor.execute(execute_params.clone()).await?;
            let result = match state {
                alkanes_cli_common::alkanes::types::ExecutionState::ReadyToSign(ready) => {
                    executor.resume_execution(ready, &execute_params).await?
                }
                _ => return Err(anyhow::anyhow!("Unexpected execution state")),
            };
            
            let txid = result.reveal_txid.clone();
            
            println!("\n✅ Swap executed!");
            println!("📝 Transaction ID: {}", txid);
            
            // Display traces if requested
            if trace {
                if let Some(all_traces) = result.traces {
                    println!("\n🔍 ═══════════════════════════════════════════════════════════════");
                    println!("🧪                   PROTOSTONE TRACES                        🧪");
                    println!("🔍 ═══════════════════════════════════════════════════════════════\n");
                    
                    for (i, trace_json) in all_traces.iter().enumerate() {
                        println!("📊 Protostone #{}", i + 1);
                        println!("───────────────────\n");
                        
                        // Debug: log the trace JSON structure
                        log::info!("Trace JSON keys: {:?}", trace_json.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                        if let Some(events) = trace_json.get("events") {
                            log::info!("Events type: {:?}, is_array: {}, array_len: {:?}", 
                                events, events.is_array(), events.as_array().map(|a| a.len()));
                        }
                        
                        if let Some(error) = trace_json.get("error") {
                            println!("   ❌ Error: {}\n", error);
                        } else {
                            // Check for events in either "trace" or "events" field
                            let events = trace_json.get("trace").and_then(|e| e.as_array())
                                .or_else(|| trace_json.get("events").and_then(|e| e.as_array()));

                            if let Some(events) = events {
                                if events.is_empty() {
                                    println!("   ⚠️  No trace data found.\n");
                                } else {
                                    // Format the trace nicely using the tree-view formatter
                                    let pretty = alkanes_cli_common::alkanes::trace::format_trace_json_pretty(trace_json);
                                    println!("{}\n", pretty);
                                }
                            } else {
                                println!("   ⚠️  Unexpected trace format: no events array\n");
                                log::warn!("Trace JSON: {}", serde_json::to_string_pretty(trace_json)?);
                            }
                        }
                    }

                    println!("🎯 ═══════════════════════════════════════════════════════════════");
                    println!("✨                      TRACE COMPLETE                         ✨");
                    println!("🎯 ═══════════════════════════════════════════════════════════════");
                } else {
                    println!("\n📭 No protostones found in this transaction.");
                }
            }
            
            Ok(())
        }
    }
}

// Helper functions for swap path optimization and simulation

/// Format a path for display
fn format_path(path: &[alkanes_cli_common::alkanes::types::AlkaneId]) -> String {
    path.iter()
        .map(|id| format!("{}:{}", id.block, id.tx))
        .collect::<Vec<_>>()
        .join(" → ")
}

/// Calculate expected output for a swap path using constant product formula
fn calculate_path_output(
    path: &[alkanes_cli_common::alkanes::types::AlkaneId],
    pools: &[alkanes_cli_common::alkanes::PoolInfo],
    input_amount: u128,
) -> Result<u128> {
    use alkanes_cli_common::alkanes::types::AlkaneId;
    
    let mut amount = input_amount;
    
    // For each hop in the path
    for i in 0..path.len() - 1 {
        let token_in = &path[i];
        let token_out = &path[i + 1];
        
        // Find the pool for this pair
        let pool = pools.iter().find(|p| {
            if let Some(details) = &p.details {
                let token_a = AlkaneId { block: details.token_a_block, tx: details.token_a_tx };
                let token_b = AlkaneId { block: details.token_b_block, tx: details.token_b_tx };
                (token_a == *token_in && token_b == *token_out) || (token_a == *token_out && token_b == *token_in)
            } else {
                false
            }
        });
        
        if let Some(pool) = pool {
            if let Some(details) = &pool.details {
                let token_a = AlkaneId { block: details.token_a_block, tx: details.token_a_tx };
                
                // Determine reserves based on token order
                let (reserve_in, reserve_out) = if token_a == *token_in {
                    (details.reserve_a, details.reserve_b)
                } else {
                    (details.reserve_b, details.reserve_a)
                };
                
                // Calculate output using constant product formula: x * y = k
                // amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
                // With 0.3% fee: amount_in_with_fee = amount_in * 997 / 1000
                let amount_in_with_fee = amount * 997 / 1000;
                let numerator = amount_in_with_fee * reserve_out;
                let denominator = reserve_in + amount_in_with_fee;
                
                if denominator == 0 {
                    return Err(anyhow::anyhow!("Pool has zero liquidity"));
                }
                
                amount = numerator / denominator;
            } else {
                return Err(anyhow::anyhow!("Pool details not available"));
            }
        } else {
            return Err(anyhow::anyhow!("No pool found for pair {}:{} -> {}:{}", 
                                      token_in.block, token_in.tx, token_out.block, token_out.tx));
        }
    }
    
    Ok(amount)
}

/// Find a direct pool between two tokens and calculate output
fn find_direct_pool(
    token_in: &alkanes_cli_common::alkanes::types::AlkaneId,
    token_out: &alkanes_cli_common::alkanes::types::AlkaneId,
    pools: &[alkanes_cli_common::alkanes::PoolInfo],
    input_amount: u128,
) -> Option<u128> {
    use alkanes_cli_common::alkanes::types::AlkaneId;
    
    for pool in pools {
        if let Some(details) = &pool.details {
            let token_a = AlkaneId { block: details.token_a_block, tx: details.token_a_tx };
            let token_b = AlkaneId { block: details.token_b_block, tx: details.token_b_tx };
            
            if (token_a == *token_in && token_b == *token_out) || (token_a == *token_out && token_b == *token_in) {
                let (reserve_in, reserve_out) = if token_a == *token_in {
                    (details.reserve_a, details.reserve_b)
                } else {
                    (details.reserve_b, details.reserve_a)
                };
                
                let amount_in_with_fee = input_amount * 997 / 1000;
                let numerator = amount_in_with_fee * reserve_out;
                let denominator = reserve_in + amount_in_with_fee;
                
                if denominator > 0 {
                    return Some(numerator / denominator);
                }
            }
        }
    }
    
    None
}

/// Build a wrap protostone that wraps BTC → frBTC
/// Calls frBTC (32:0) opcode 77 (exchange/wrap)
fn build_wrap_protostone(
    amount: u64,
    pointer_output: u32,
) -> alkanes_cli_common::alkanes::types::ProtostoneSpec {
    use alkanes_cli_common::alkanes::types::{ProtostoneSpec, OutputTarget, BitcoinTransfer};
    use alkanes_support::id::AlkaneId as SupportAlkaneId;
    
    ProtostoneSpec {
        cellpack: Some(alkanes_support::cellpack::Cellpack {
            target: SupportAlkaneId {
                block: 32,  // frBTC
                tx: 0,
            },
            inputs: vec![77],  // Opcode 77: exchange/wrap
        }),
        edicts: vec![],  // No edicts, minted frBTC goes to pointer destination
        bitcoin_transfer: Some(BitcoinTransfer {
            amount,
            target: OutputTarget::Output(0),  // Send BTC to subfrost address (output 0)
        }),
        pointer: Some(OutputTarget::Output(pointer_output)),  // Minted frBTC destination
        refund: Some(OutputTarget::Output(pointer_output)),   // Refund unused frBTC
    }
}

/// Build an unwrap protostone that unwraps frBTC → BTC
/// Calls frBTC (32:0) opcode 78 (unwrap) with dust output in cellpack
fn build_unwrap_protostone(
    unwrap_amount: u128,
    dust_vout: u32,
    btc_recipient_vout: u32,
    refund_vout: u32,
) -> alkanes_cli_common::alkanes::types::ProtostoneSpec {
    use alkanes_cli_common::alkanes::types::{ProtostoneSpec, OutputTarget};
    use alkanes_support::id::AlkaneId as SupportAlkaneId;
    
    // Calldata: [32, 0, 78, dust_vout, unwrap_amount]
    let calldata = vec![32u128, 0u128, 78u128, dust_vout as u128, unwrap_amount];
    
    ProtostoneSpec {
        cellpack: Some(alkanes_support::cellpack::Cellpack {
            target: SupportAlkaneId {
                block: 32,  // frBTC
                tx: 0,
            },
            inputs: calldata,
        }),
        edicts: vec![],  // frBTC comes from previous protostone's pointer
        bitcoin_transfer: None,  // No BTC transfer in unwrap, BTC is output from the unwrap itself
        pointer: Some(OutputTarget::Output(btc_recipient_vout)),  // Where unwrapped BTC goes
        refund: Some(OutputTarget::Output(refund_vout)),  // Where frBTC goes if unwrap fails
    }
}

/// Simulate a swap to get the expected output using factory routing
async fn simulate_swap_output<T: System>(
    system: &mut T,
    factory: &str,
    path: &[alkanes_cli_common::alkanes::types::AlkaneId],
    input_amount: u128,
    minimum_output: u128,
    expires: u64,
    current_height: u64,
) -> Result<u128> {
    use alkanes_cli_common::proto::alkanes::{MessageContextParcel, SimulateResponse};
    use alkanes_cli_common::traits::MetashrewRpcProvider;
    use prost::Message;
    
    // Parse factory
    let factory_parts: Vec<&str> = factory.split(':').collect();
    let factory_block: u64 = factory_parts[0].parse()?;
    let factory_tx: u64 = factory_parts[1].parse()?;
    
    // Build calldata for factory swap simulation (opcode 13)
    // Format: [factory_block, factory_tx, 13, path_length, token0_block, token0_tx, ..., amount_in, min_out, deadline]
    let mut calldata = Vec::new();
    leb128::write::unsigned(&mut calldata, factory_block).unwrap();
    leb128::write::unsigned(&mut calldata, factory_tx).unwrap();
    leb128::write::unsigned(&mut calldata, 13u64).unwrap(); // swap_exact_tokens_for_tokens opcode
    leb128::write::unsigned(&mut calldata, path.len() as u64).unwrap();
    
    // Add path tokens
    for token in path {
        leb128::write::unsigned(&mut calldata, token.block).unwrap();
        leb128::write::unsigned(&mut calldata, token.tx).unwrap();
    }
    
    // Add amount_in, min_out, deadline
    leb128::write::unsigned(&mut calldata, input_amount as u64).unwrap();
    leb128::write::unsigned(&mut calldata, minimum_output as u64).unwrap();
    leb128::write::unsigned(&mut calldata, expires).unwrap();
    
    let context = MessageContextParcel {
        alkanes: vec![],
        transaction: vec![],
        block: vec![],
        height: current_height,
        vout: 0,
        txindex: 1,
        calldata,
        pointer: 0,
        refund_pointer: 0,
    };
    
    let result = system.provider().simulate(factory, &context, None).await?;
    let hex_data = result.as_str().ok_or_else(|| anyhow::anyhow!("Expected string result"))?;
    let hex_data = hex_data.strip_prefix("0x").unwrap_or(hex_data);
    let bytes = hex::decode(hex_data)?;
    let sim_response = SimulateResponse::decode(bytes.as_slice())?;
    
    if let Some(execution) = &sim_response.execution {
        // The output tokens should be in the execution result
        // The factory returns the output token in the alkanes field
        if !execution.alkanes.is_empty() {
            // Find the output token (last token in path) in the returned alkanes
            let output_token = &path[path.len() - 1];
            for alkane in &execution.alkanes {
                if let Some(alkane_id) = &alkane.id {
                    // Extract block and tx values
                    let block_val = alkane_id.block.as_ref().map(|b| (b.hi as u128) << 64 | b.lo as u128).unwrap_or(0);
                    let tx_val = alkane_id.tx.as_ref().map(|t| (t.hi as u128) << 64 | t.lo as u128).unwrap_or(0);
                    
                    if block_val == output_token.block as u128 && tx_val == output_token.tx as u128 {
                        if let Some(value) = &alkane.value {
                            return Ok((value.hi as u128) << 64 | value.lo as u128);
                        }
                    }
                }
            }
        }
        
        // If not found in alkanes, it might be in the data field
        // The contract might return just the amount as u128
        if execution.data.len() >= 16 {
            let output = u128::from_le_bytes(execution.data[0..16].try_into().unwrap());
            return Ok(output);
        }
    }
    
    Err(anyhow::anyhow!("Could not extract output amount from simulation"))
}

/// Get pool details for a specific pool
async fn get_pool_details<T: System>(
    system: &mut T,
    pool_block: u64,
    pool_tx: u64,
    current_height: u64,
) -> Result<alkanes_cli_common::alkanes::PoolDetails> {
    use alkanes_cli_common::proto::alkanes::{MessageContextParcel, SimulateResponse};
    use alkanes_cli_common::traits::MetashrewRpcProvider;
    use alkanes_cli_common::alkanes::PoolDetails;
    use prost::Message;
    
    let mut detail_calldata = Vec::new();
    leb128::write::unsigned(&mut detail_calldata, pool_block).unwrap();
    leb128::write::unsigned(&mut detail_calldata, pool_tx).unwrap();
    leb128::write::unsigned(&mut detail_calldata, 999u64).unwrap();
    
    let detail_context = MessageContextParcel {
        alkanes: vec![],
        transaction: vec![],
        block: vec![],
        height: current_height,
        vout: 0,
        txindex: 1,
        calldata: detail_calldata,
        pointer: 0,
        refund_pointer: 0,
    };
    
    let detail_result = system.provider().simulate(&format!("{}:{}", pool_block, pool_tx), &detail_context, None).await?;
    let detail_hex = detail_result.as_str().ok_or_else(|| anyhow::anyhow!("Expected string result"))?;
    let detail_hex = detail_hex.strip_prefix("0x").unwrap_or(detail_hex);
    let detail_bytes = hex::decode(detail_hex)?;
    let detail_sim = SimulateResponse::decode(detail_bytes.as_slice())?;
    
    if let Some(detail_exec) = &detail_sim.execution {
        PoolDetails::from_bytes(&detail_exec.data)
    } else {
        Err(anyhow::anyhow!("No execution result in simulation"))
    }
}

/// Find the pool ID for a given token pair
async fn find_pool_for_pair<T: System>(
    token_a: &alkanes_cli_common::alkanes::types::AlkaneId,
    token_b: &alkanes_cli_common::alkanes::types::AlkaneId,
    system: &mut T,
    factory: &str,
    current_height: u64,
) -> Result<(u64, u64)> {
    use alkanes_cli_common::proto::alkanes::{MessageContextParcel, SimulateResponse};
    use alkanes_cli_common::traits::MetashrewRpcProvider;
    use alkanes_cli_common::alkanes::PoolDetails;
    use prost::Message;
    
    // Parse factory
    let factory_parts: Vec<&str> = factory.split(':').collect();
    let factory_block: u64 = factory_parts[0].parse()?;
    let factory_tx: u64 = factory_parts[1].parse()?;
    
    // Get all pools
    let mut pool_calldata = Vec::new();
    leb128::write::unsigned(&mut pool_calldata, factory_block).unwrap();
    leb128::write::unsigned(&mut pool_calldata, factory_tx).unwrap();
    leb128::write::unsigned(&mut pool_calldata, 3u64).unwrap(); // GET_ALL_POOLS opcode
    
    let context = MessageContextParcel {
        alkanes: vec![],
        transaction: vec![],
        block: vec![],
        height: current_height,
        vout: 0,
        txindex: 1,
        calldata: pool_calldata,
        pointer: 0,
        refund_pointer: 0,
    };
    
    let result = system.provider().simulate(factory, &context, None).await?;
    let hex_data = result.as_str().ok_or_else(|| anyhow::anyhow!("Expected string result"))?;
    let hex_data = hex_data.strip_prefix("0x").unwrap_or(hex_data);
    let bytes = hex::decode(hex_data)?;
    let sim_response = SimulateResponse::decode(bytes.as_slice())?;
    
    let pools = if let Some(execution) = &sim_response.execution {
        let data = &execution.data;
        if data.len() < 16 {
            return Err(anyhow::anyhow!("Invalid response from factory"));
        }
        
        let count = u128::from_le_bytes(data[0..16].try_into().unwrap()) as usize;
        let mut pools = Vec::new();
        
        for i in 0..count {
            let offset = 16 + (i * 32);
            if offset + 32 > data.len() {
                break;
            }
            let pool_block = u128::from_le_bytes(data[offset..offset+16].try_into().unwrap()) as u64;
            let pool_tx = u128::from_le_bytes(data[offset+16..offset+32].try_into().unwrap()) as u64;
            pools.push((pool_block, pool_tx));
        }
        pools
    } else {
        return Err(anyhow::anyhow!("No pools found"));
    };
    
    // Check each pool to find the one with our token pair
    for (pool_block, pool_tx) in pools {
        let mut detail_calldata = Vec::new();
        leb128::write::unsigned(&mut detail_calldata, pool_block).unwrap();
        leb128::write::unsigned(&mut detail_calldata, pool_tx).unwrap();
        leb128::write::unsigned(&mut detail_calldata, 999u64).unwrap();
        
        let detail_context = MessageContextParcel {
            alkanes: vec![],
            transaction: vec![],
            block: vec![],
            height: current_height,
            vout: 0,
            txindex: 1,
            calldata: detail_calldata,
            pointer: 0,
            refund_pointer: 0,
        };
        
        if let Ok(detail_result) = system.provider().simulate(&format!("{}:{}", pool_block, pool_tx), &detail_context, None).await {
            if let Some(detail_hex) = detail_result.as_str() {
                let detail_hex = detail_hex.strip_prefix("0x").unwrap_or(detail_hex);
                if let Ok(detail_bytes) = hex::decode(detail_hex) {
                    if let Ok(detail_sim) = SimulateResponse::decode(detail_bytes.as_slice()) {
                        if let Some(detail_exec) = &detail_sim.execution {
                            if let Ok(details) = PoolDetails::from_bytes(&detail_exec.data) {
                                let pool_token_a = alkanes_cli_common::alkanes::types::AlkaneId {
                                    block: details.token_a_block,
                                    tx: details.token_a_tx,
                                };
                                let pool_token_b = alkanes_cli_common::alkanes::types::AlkaneId {
                                    block: details.token_b_block,
                                    tx: details.token_b_tx,
                                };
                                
                                if (pool_token_a == *token_a && pool_token_b == *token_b) ||
                                   (pool_token_a == *token_b && pool_token_b == *token_a) {
                                    return Ok((pool_block, pool_tx));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("No pool found for pair {}:{} / {}:{}", 
                        token_a.block, token_a.tx, token_b.block, token_b.tx))
}

async fn backtest_transaction<T: System>(system: &mut T, txid: &str, raw: bool) -> Result<()> {
    use bitcoin::consensus::{Decodable, Encodable};
    use bitcoin::Block;
    use bitcoin::hashes::Hash;
    use std::io::Cursor;
    use std::str::FromStr;
    use alkanes_cli_common::traits::BitcoinRpcProvider;
    
    // Step 1: Fetch the transaction hex
    println!("📥 Fetching transaction {}...", txid);
    let tx_hex = system.provider().get_tx_hex(txid).await?;
    
    // Decode the transaction to get details
    let tx_bytes = hex::decode(&tx_hex)?;
    let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
    
    // Step 2: Get the current block height to determine which block to query before
    let current_height = system.provider().get_block_count().await?;
    let block_tag_before = if current_height > 0 {
        (current_height - 1).to_string()
    } else {
        "0".to_string()
    };
    
    println!("📊 Creating simulated block...");
    println!("   Current height: {}", current_height);
    println!("   Querying state at height: {}", block_tag_before);
    
    // Step 3: Build a dummy block with a coinbase and our transaction
    // Get the previous block hash
    let prev_block_hash = if current_height > 0 {
        let prev_hash_str = BitcoinRpcProvider::get_block_hash(system.provider(), current_height - 1).await?;
        bitcoin::BlockHash::from_str(&prev_hash_str)?
    } else {
        bitcoin::BlockHash::from_byte_array([0u8; 32])
    };
    
    // Create a simple coinbase transaction
    let coinbase_tx = bitcoin::Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint::null(),
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: bitcoin::Witness::from_slice(&[vec![0u8; 32]]),
        }],
        output: vec![bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(50_00000000),
            script_pubkey: bitcoin::ScriptBuf::new(),
        }],
    };
    
    // Create the block with coinbase and our transaction
    let simulated_block = Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::TWO,
            prev_blockhash: prev_block_hash,
            merkle_root: bitcoin::TxMerkleNode::from_byte_array([0u8; 32]),
            time: (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32),
            bits: bitcoin::CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        },
        txdata: vec![coinbase_tx, tx.clone()],
    };
    
    // Encode the block to hex
    let mut block_bytes = Vec::new();
    simulated_block.consensus_encode(&mut block_bytes)?;
    let block_hex = hex::encode(&block_bytes);
    
    println!("✅ Simulated block created ({} bytes)", block_bytes.len());
    println!("   Transactions: coinbase + {}", txid);
    
    // Step 4: Build the trace input data
    // The trace view function expects:
    // 1. Height (u32) - 4 bytes
    // 2. Protobuf-encoded Outpoint message (txid + vout)
    println!("🔍 Preparing trace input data...");
    let txid_bytes = hex::decode(txid)
        .map_err(|e| anyhow::anyhow!("Invalid txid hex: {}", e))?;
    
    if txid_bytes.len() != 32 {
        return Err(anyhow::anyhow!("TXID must be 32 bytes, got {}", txid_bytes.len()));
    }
    
    let vout: u32 = 0; // Trace the first output
    
    // Create protobuf Outpoint message
    use protorune_support::proto::protorune::Outpoint;
    use prost::Message;
    
    let outpoint = Outpoint {
        txid: txid_bytes.clone(),
        vout,
    };
    
    let outpoint_bytes = outpoint.encode_to_vec();
    
    // Input data format: height (4 bytes) + protobuf outpoint
    let height_u32: u32 = block_tag_before.parse()
        .map_err(|e| anyhow::anyhow!("Invalid height: {}", e))?;
    
    let mut input_data = Vec::new();
    input_data.extend_from_slice(&height_u32.to_le_bytes());  // Height as u32 LE
    input_data.extend_from_slice(&outpoint_bytes);
    
    let input_data_hex = hex::encode(&input_data);
    println!("   Height: {}", height_u32);
    println!("   Outpoint: {}:{}", txid, vout);
    println!("   Input data: {} bytes (height + protobuf outpoint)", input_data.len());
    
    // Step 5: Call metashrew_preview
    println!("🔍 Calling metashrew_preview...");
    
    let params = serde_json::json!([
        block_hex,
        "trace",
        input_data_hex,
        block_tag_before
    ]);
    
    use alkanes_cli_common::traits::JsonRpcProvider;
    let metashrew_url = system.provider().get_metashrew_rpc_url()
        .ok_or_else(|| anyhow::anyhow!("Metashrew RPC URL not configured"))?;
    let result = system.provider().call(
        &metashrew_url,
        "metashrew_preview",
        params,
        1
    ).await?;
    
    if raw {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }
    
    // Step 6: Pretty print the transaction analysis
    println!("\n{} Transaction Analysis {}\n", "═".repeat(30), "═".repeat(30));
    pretty_print_transaction_analysis(&tx);
    
    // Step 7: Parse and print the trace
    println!("\n{} Execution Trace {}\n", "═".repeat(30), "═".repeat(30));
    
    if let Some(trace_data) = result.get("trace") {
        // Try to parse as protobuf trace
        if let Some(trace_hex) = trace_data.as_str() {
            let trace_hex = trace_hex.strip_prefix("0x").unwrap_or(trace_hex);
            if let Ok(trace_bytes) = hex::decode(trace_hex) {
                // Try to decode as alkanes trace
                if let Ok(trace) = alkanes_support::trace::Trace::try_from(trace_bytes) {
                    let trace_json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                    println!("{}", serde_json::to_string_pretty(&trace_json)?);
                } else {
                    println!("⚠️  Could not decode trace protobuf");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        } else {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    
    Ok(())
}

fn pretty_print_transaction_analysis(tx: &bitcoin::Transaction) {
    use colored::Colorize;
    
    println!("📋 {} {}", "Transaction ID:".bold(), tx.txid().to_string().bright_cyan());
    println!("📏 {} {} bytes", "Size:".bold(), tx.total_size());
    println!("⚖️  {} {} WU", "Weight:".bold(), tx.weight().to_wu());
    println!("🔢 {} {}", "Version:".bold(), tx.version.0);
    println!("🔒 {} {}", "Locktime:".bold(), tx.lock_time);
    
    println!("\n{} {} inputs", "Inputs:".bold().bright_blue(), tx.input.len());
    for (i, input) in tx.input.iter().enumerate() {
        if input.previous_output.is_null() {
            println!("  {} {} {}", format!("{}.", i).dimmed(), "⛏️".bold(), "Coinbase".bright_green());
        } else {
            println!("  {} {}:{}", 
                format!("{}.", i).dimmed(),
                &input.previous_output.txid.to_string()[..16],
                input.previous_output.vout
            );
            if !input.witness.is_empty() {
                let witness_size: usize = input.witness.iter().map(|w| w.len()).sum();
                println!("     {} {} items, {} bytes", 
                    "Witness:".dimmed(),
                    input.witness.len(),
                    witness_size.to_string().bright_yellow()
                );
            }
        }
    }
    
    println!("\n{} {} outputs", "Outputs:".bold().bright_blue(), tx.output.len());
    for (i, output) in tx.output.iter().enumerate() {
        let output_icon = if output.script_pubkey.is_op_return() {
            "📝"
        } else if output.script_pubkey.is_p2tr() {
            "🔑"
        } else if output.script_pubkey.is_witness_program() {
            "⚡"
        } else {
            "📤"
        };
        
        println!("  {} {} {} sats",
            format!("{}.", i).dimmed(),
            output_icon,
            output.value.to_sat().to_string().bright_yellow()
        );
        
        if output.script_pubkey.is_op_return() {
            println!("     {} OP_RETURN data", "📝".dimmed());
        }
    }
}

fn to_enhanced_execute_params(args: AlkanesExecute) -> Result<alkanes::types::EnhancedExecuteParams> {
    let input_requirements = args.inputs.map(|s| alkanes::parsing::parse_input_requirements(&s)).transpose()?.unwrap_or_default();
    let protostones = alkanes::parsing::parse_protostones(&args.protostones.join(" "))?;
    let envelope_data = args.envelope.map(std::fs::read).transpose()?;

    Ok(alkanes::types::EnhancedExecuteParams {
        input_requirements,
        to_addresses: args.to,
        from_addresses: args.from,
        change_address: args.change,
        alkanes_change_address: args.alkanes_change,
        fee_rate: args.fee_rate,
        envelope_data,
        protostones,
        raw_output: args.raw,
        trace_enabled: args.trace,
        mine_enabled: args.mine,
        auto_confirm: args.auto_confirm,
        ordinals_strategy: alkanes::types::OrdinalsStrategy::default(),
        mempool_indexer: false,
    })
}

async fn execute_runestone_command<T: System>(system: &mut T, command: Runestone) -> Result<()> {
    match command {
        Runestone::Analyze { txid, raw } => {
            let network = system.provider().get_network();
            let tx_hex = system.provider().get_transaction_hex(&txid).await?;
            let tx_bytes = hex::decode(tx_hex)?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx, network)?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                alkanes_cli_common::runestone_enhanced::print_human_readable_runestone(&tx, &result, network);
            }
        }
        Runestone::Trace { txid, raw } => {
            use alkanes_cli_common::traits::AlkanesProvider;

            // Get and analyze transaction
            let network = system.provider().get_network();
            let tx_hex = system.provider().get_transaction_hex(&txid).await?;
            let tx_bytes = hex::decode(tx_hex)?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx, network)?;

            // Print transaction structure
            if !raw {
                println!("🔍 ═══════════════════════════════════════════════════════════════");
                println!("🧪           RUNESTONE TRANSACTION TRACE ANALYSIS             🧪");
                println!("🔍 ═══════════════════════════════════════════════════════════════\n");
                println!("📝 Transaction ID: {}\n", txid);
                alkanes_cli_common::runestone_enhanced::print_human_readable_runestone(&tx, &result, network);
                println!("\n🔍 ═══════════════════════════════════════════════════════════════");
                println!("🧪                   PROTOSTONE TRACES                        🧪");
                println!("🔍 ═══════════════════════════════════════════════════════════════\n");
            }
            
            // Use the abstracted trace_protostones method
            let traces_opt = system.provider().trace_protostones(&txid).await?;
            
            if let Some(all_traces) = traces_opt {
                if raw {
                    let output = serde_json::json!({
                        "transaction": result,
                        "traces": all_traces
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else {
                    // Print each trace nicely
                    for (i, trace_json) in all_traces.iter().enumerate() {
                        let vout = tx.output.len() as u32 + 1 + i as u32;
                        println!("📊 Protostone #{} (virtual vout {}):", i + 1, vout);
                        println!("   Outpoint: {}:{}\n", txid, vout);
                        
                        if let Some(error) = trace_json.get("error") {
                            println!("   ❌ Error: {}\n", error);
                        } else {
                            // Check for events in either "trace" or "events" field
                            let events = trace_json.get("trace").and_then(|e| e.as_array())
                                .or_else(|| trace_json.get("events").and_then(|e| e.as_array()));

                            if let Some(events) = events {
                                if events.is_empty() {
                                    println!("   ⚠️ No trace data found.\n");
                                } else {
                                    // Format the trace nicely using the tree-view formatter
                                    let pretty = alkanes_cli_common::alkanes::trace::format_trace_json_pretty(trace_json);
                                    println!("{}\n", pretty);
                                }
                            } else {
                                println!("   ⚠️ Unexpected trace format.\n");
                            }
                        }
                    }

                    println!("🎯 ═══════════════════════════════════════════════════════════════");
                    println!("✨                      TRACE COMPLETE                         ✨");
                    println!("🎯 ═══════════════════════════════════════════════════════════════");
                }
            } else {
                if raw {
                    println!("{{\"transaction\": {}, \"traces\": []}}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("📭 No protostones found in this transaction.\n");
                }
            }
        }
    }
    Ok(())
}



async fn execute_esplora_command(
    provider: &dyn DeezelProvider,
    command: alkanes_cli_common::commands::EsploraCommands,
) -> anyhow::Result<()> {
    match command {
        alkanes_cli_common::commands::EsploraCommands::BlocksTipHash { raw } => {
            let hash = provider.get_blocks_tip_hash().await?;
            if raw {
                println!("{hash}");
            } else {
                println!("⛓️ Tip Hash: {hash}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlocksTipHeight { raw } => {
            let height = provider.get_blocks_tip_height().await?;
            if raw {
                println!("{height}");
            } else {
                println!("📈 Tip Height: {height}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::Blocks { start_height, raw } => {
            let result = provider.get_blocks(start_height).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("📦 Blocks:\n{}", serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockHeight { height, raw } => {
            let hash = provider.get_block_by_height(height).await?;
            if raw {
                println!("{hash}");
            } else {
                println!("🔗 Block Hash at {height}: {hash}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::Block { hash, raw } => {
            let block = <dyn EsploraProvider>::get_block(provider, &hash).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&block)?);
            } else {
                println!("📦 Block {}:\n{}", hash, serde_json::to_string_pretty(&block)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockStatus { hash, raw } => {
            let status = provider.get_block_status(&hash).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("ℹ️ Block Status {}:\n{}", hash, serde_json::to_string_pretty(&status)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockTxids { hash, raw } => {
            let txids = provider.get_block_txids(&hash).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&txids)?);
            } else {
                println!("📄 Block Txids {}:\n{}", hash, serde_json::to_string_pretty(&txids)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockHeader { hash, raw } => {
            let header = alkanes_cli_common::traits::EsploraProvider::get_block_header(provider, &hash).await?;
            if raw {
                println!("{header}");
            } else {
                println!("📄 Block Header {hash}: {header}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockRaw { hash, raw } => {
            let raw_block = provider.get_block_raw(&hash).await?;
            if raw {
                println!("{raw_block}");
            } else {
                println!("📦 Raw Block {hash}: {raw_block}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockTxid { hash, index, raw } => {
            let txid = provider.get_block_txid(&hash, index).await?;
            if raw {
                println!("{txid}");
            } else {
                println!("📄 Txid at index {index} in block {hash}: {txid}");
            }
        }
        alkanes_cli_common::commands::EsploraCommands::BlockTxs { hash, start_index, raw } => {
            let txs = provider.get_block_txs(&hash, start_index).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&txs)?);
            } else {
                println!("📄 Transactions in block {}:\n{}", hash, serde_json::to_string_pretty(&txs)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::Address { params, raw } => {
            let resolved_address = provider.resolve_all_identifiers(&params).await?;
            let result = provider.get_address_info(&resolved_address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🏠 Address {}:\n{}", params, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxs { params, raw, exclude_coinbase, runestone_trace } => {
            let resolved_address = provider.resolve_all_identifiers(&params).await?;
            let result = provider.get_address_txs(&resolved_address).await?;
            
            // Parse JSON result into EsploraTransaction structs
            let mut txs: Vec<alkanes_cli_common::esplora::EsploraTransaction> = serde_json::from_value(result)
                .map_err(|e| anyhow::anyhow!("Failed to parse transactions: {}", e))?;
            
            // Filter out coinbase transactions if requested
            if exclude_coinbase {
                txs.retain(|tx| !tx.vin.iter().any(|vin| vin.is_coinbase));
            }
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&txs)?);
            } else {
                println!("📄 Transactions for address {}:", params);
                pretty_print::print_esplora_transactions(&txs);
            }
            
            // If runestone_trace flag is set, trace each transaction that has an OP_RETURN
            if runestone_trace {
                use alkanes_cli_common::traits::AlkanesProvider;
                use prost::Message;

                let network = provider.get_network();
                println!("\n🔍 ═══════════════════════════════════════════════════════════════");
                println!("🧪            RUNESTONE TRACES FOR TRANSACTIONS               🧪");
                println!("🔍 ═══════════════════════════════════════════════════════════════\n");

                for esplora_tx in &txs {
                    // Check if transaction has an OP_RETURN output
                    let has_op_return = esplora_tx.vout.iter().any(|output| {
                        output.scriptpubkey_type == "op_return"
                    });
                    
                    if has_op_return {
                        println!("═══════════════════════════════════════════════════════════════");
                        println!("📝 Transaction: {}", esplora_tx.txid);
                        println!("═══════════════════════════════════════════════════════════════\n");
                        
                        // Get raw transaction
                        match provider.get_transaction_hex(&esplora_tx.txid).await {
                            Ok(tx_hex) => {
                                let tx_bytes = match hex::decode(&tx_hex) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        println!("❌ Error decoding hex: {}", e);
                                        continue;
                                    }
                                };
                                
                                let transaction: bitcoin::Transaction = match bitcoin::consensus::deserialize(&tx_bytes) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        println!("❌ Error deserializing transaction: {}", e);
                                        continue;
                                    }
                                };
                                
                                // Try to parse runestone
                                match alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&transaction, network) {
                                    Ok(result) => {
                                        // Extract number of protostones
                                        let num_protostones = if let Some(protostones) = result.get("protostones").and_then(|p| p.as_array()) {
                                            protostones.len()
                                        } else {
                                            0
                                        };
                                        
                                        if num_protostones == 0 {
                                            println!("ℹ️ No protostones found in this transaction.\n");
                                            continue;
                                        }
                                        
                                        println!("🪨 Protostones Found: {}\n", num_protostones);
                                        
                                        // Trace each protostone
                                        let base_vout = transaction.output.len() as u32 + 1;
                                        for i in 0..num_protostones {
                                            let vout = base_vout + i as u32;
                                            let outpoint = format!("{}:{}", esplora_tx.txid, vout);
                                            
                                            println!("📊 Protostone #{} (virtual vout {}):", i + 1, vout);
                                            println!("   Outpoint: {}\n", outpoint);
                                            
                                            match provider.trace(&outpoint).await {
                                                Ok(trace_pb) => {
                                                    if let Some(alkanes_trace) = trace_pb.trace {
                                                        // Convert and pretty print trace
                                                        match alkanes_support::trace::Trace::try_from(
                                                            prost::Message::encode_to_vec(&alkanes_trace)
                                                        ) {
                                                            Ok(trace) => {
                                                                let formatted = alkanes_cli_common::alkanes::trace::format_trace_pretty(&trace);
                                                                println!("{}", formatted);
                                                            }
                                                            Err(e) => {
                                                                println!("   ❌ Error decoding trace: {}", e);
                                                            }
                                                        }
                                                    } else {
                                                        println!("   ⚠️ No trace data found.");
                                                    }
                                                }
                                                Err(e) => {
                                                    println!("   ❌ Error tracing: {}", e);
                                                }
                                            }
                                            println!();
                                        }
                                    }
                                    Err(e) => {
                                        println!("ℹ️ Not a valid runestone: {}\n", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("❌ Error fetching transaction: {}", e);
                            }
                        }
                        println!();
                    }
                }
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxsChain { params, raw } => {
            let resolved_address = provider.resolve_all_identifiers(&params).await?;
            let result = provider.get_address_txs_chain(&resolved_address, None).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("⛓️ Chain transactions for address {}:\n{}", params, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxsMempool { address, raw } => {
            let resolved_address = provider.resolve_all_identifiers(&address).await?;
            let result = provider.get_address_txs_mempool(&resolved_address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("⏳ Mempool transactions for address {}:\n{}", address, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressUtxo { address, raw } => {
            let resolved_address = provider.resolve_all_identifiers(&address).await?;
            let result = provider.get_address_utxo(&resolved_address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("💰 UTXOs for address {}:\n{}", address, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressPrefix { prefix, raw } => {
            let result = provider.get_address_prefix(&prefix).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🔍 Addresses with prefix '{}':\n{}", prefix, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::Tx { txid, raw } => {
            let tx = provider.get_tx(&txid).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&tx)?);
            } else {
                println!("📄 Transaction {}:\n{}", txid, serde_json::to_string_pretty(&tx)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::TxHex { txid, .. } => {
            let hex = provider.get_tx_hex(&txid).await?;
            println!("{hex}");
        }
        alkanes_cli_common::commands::EsploraCommands::TxRaw { txid, .. } => {
            let raw_tx = provider.get_tx_raw(&txid).await?;
            println!("{}", hex::encode(raw_tx));
        }
        alkanes_cli_common::commands::EsploraCommands::TxStatus { txid, raw } => {
            let status = provider.get_tx_status(&txid).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("ℹ️ Status for tx {}:\n{}", txid, serde_json::to_string_pretty(&status)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::TxMerkleProof { txid, raw } => {
            let proof = provider.get_tx_merkle_proof(&txid).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&proof)?);
            } else {
                println!("🧾 Merkle proof for tx {}:\n{}", txid, serde_json::to_string_pretty(&proof)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::TxMerkleblockProof { txid, .. } => {
            let proof = provider.get_tx_merkleblock_proof(&txid).await?;
            println!("{proof}");
        }
        alkanes_cli_common::commands::EsploraCommands::TxOutspend { txid, index, raw } => {
            let outspend = provider.get_tx_outspend(&txid, index).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&outspend)?);
            } else {
                println!("💸 Outspend for tx {}, vout {}:\n{}", txid, index, serde_json::to_string_pretty(&outspend)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::TxOutspends { txid, raw } => {
            let outspends = provider.get_tx_outspends(&txid).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&outspends)?);
            } else {
                println!("💸 Outspends for tx {}:\n{}", txid, serde_json::to_string_pretty(&outspends)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::Broadcast { tx_hex, .. } => {
            let txid = provider.broadcast(&tx_hex).await?;
            println!("✅ Transaction broadcast successfully!");
            println!("🔗 Transaction ID: {txid}");
        }
        alkanes_cli_common::commands::EsploraCommands::PostTx { tx_hex, .. } => {
            let txid = provider.broadcast(&tx_hex).await?;
            println!("✅ Transaction posted successfully!");
            println!("🔗 Transaction ID: {txid}");
        }
        alkanes_cli_common::commands::EsploraCommands::Mempool { raw } => {
            let mempool = provider.get_mempool().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&mempool)?);
            } else {
                println!("⏳ Mempool Info:\n{}", serde_json::to_string_pretty(&mempool)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::MempoolTxids { raw } => {
            let txids = provider.get_mempool_txids().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&txids)?);
            } else {
                println!("📄 Mempool Txids:\n{}", serde_json::to_string_pretty(&txids)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::MempoolRecent { raw } => {
            let recent = provider.get_mempool_recent().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&recent)?);
            } else {
                println!("📄 Recent Mempool Txs:\n{}", serde_json::to_string_pretty(&recent)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::FeeEstimates { raw } => {
            let estimates = provider.get_fee_estimates().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&estimates)?);
            } else {
                println!("💰 Fee Estimates:\n{}", serde_json::to_string_pretty(&estimates)?);
            }
        }
    }
    Ok(())
}

async fn execute_ord_command(
    provider: &dyn DeezelProvider,
    command: alkanes_cli_common::commands::OrdCommands,
) -> anyhow::Result<()> {
    match command {
        alkanes_cli_common::commands::OrdCommands::Inscription { id, raw } => {
            if raw {
                let inscription = provider.get_inscription(&id).await?;
                let json_value = serde_json::to_value(&inscription)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let inscription = provider.get_inscription(&id).await?;
                print_inscription(&inscription);
            }
        }
        alkanes_cli_common::commands::OrdCommands::InscriptionsInBlock { hash, raw } => {
            if raw {
                let inscriptions = provider.get_inscriptions_in_block(&hash).await?;
                let json_value = serde_json::to_value(&inscriptions)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let inscriptions = provider.get_inscriptions_in_block(&hash).await?;
                let inscription_futures = inscriptions.ids.into_iter().map(|id| {
                    let provider = provider;
                    async move { provider.get_inscription(&id.to_string()).await }
                });
                let results: Vec<_> = join_all(inscription_futures).await;
                let fetched_inscriptions: Result<Vec<_>, _> = results.into_iter().collect();
                print_inscriptions(&fetched_inscriptions?);
            }
        }
        alkanes_cli_common::commands::OrdCommands::AddressInfo { address, raw } => {
            let resolved_address = provider.resolve_all_identifiers(&address).await?;
            if raw {
                let info = provider.get_ord_address_info(&resolved_address).await?;
                let json_value = serde_json::to_value(&info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let info = provider.get_ord_address_info(&resolved_address).await?;
                print_address_info(&info);
            }
        }
        alkanes_cli_common::commands::OrdCommands::BlockInfo { query, raw } => {
            if raw {
                let info = provider.get_block_info(&query).await?;
                let json_value = serde_json::to_value(&info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let info = provider.get_block_info(&query).await?;
                if let Some(info) = info.info {
                    print_block_info(&info);
                } else {
                    println!("Block info not available.");
                }
            }
        }
        alkanes_cli_common::commands::OrdCommands::BlockCount => {
            let info = provider.get_ord_block_count().await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        alkanes_cli_common::commands::OrdCommands::Blocks { raw } => {
            if raw {
                let info = provider.get_ord_blocks().await?;
                let json_value = serde_json::to_value(&info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let info = provider.get_ord_blocks().await?;
                print_blocks(&info);
            }
        }
        alkanes_cli_common::commands::OrdCommands::Children { id, page, raw } => {
            if raw {
                let children = provider.get_children(&id, page).await?;
                let json_value = serde_json::to_value(&children)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let children = provider.get_children(&id, page).await?;
                let inscription_futures = children.ids.into_iter().map(|id| {
                    let provider = provider;
                    async move { provider.get_inscription(&id.to_string()).await }
                });
                let results: Vec<_> = join_all(inscription_futures).await;
                let fetched_inscriptions: Result<Vec<_>, _> = results.into_iter().collect();
                print_children(&fetched_inscriptions?);
            }
        }
        alkanes_cli_common::commands::OrdCommands::Content { id } => {
            let content = provider.get_content(&id).await?;
            use std::io::{self, Write};
            io::stdout().write_all(&content)?;
        }
        alkanes_cli_common::commands::OrdCommands::Output { outpoint, raw } => {
            if raw {
                let output = provider.get_output(&outpoint).await?;
                let json_value = serde_json::to_value(&output)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let output = provider.get_output(&outpoint).await?;
                print_output(&output);
            }
        }
        alkanes_cli_common::commands::OrdCommands::Parents { id, page, raw } => {
            if raw {
                let parents = provider.get_parents(&id, page).await?;
                let json_value = serde_json::to_value(&parents)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let parents = provider.get_parents(&id, page).await?;
                print_parents(&parents);
            }
        }
        alkanes_cli_common::commands::OrdCommands::Rune { rune, raw } => {
            if raw {
                let rune_info = provider.get_rune(&rune).await?;
                let json_value = serde_json::to_value(&rune_info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let rune_info = provider.get_rune(&rune).await?;
                print_rune(&rune_info);
            }
        }
        alkanes_cli_common::commands::OrdCommands::Sat { sat, raw } => {
            if raw {
                let sat_info = provider.get_sat(sat).await?;
                let json_value = serde_json::to_value(&sat_info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let sat_info = provider.get_sat(sat).await?;
                print_sat_response(&sat_info);
            }
        }
        alkanes_cli_common::commands::OrdCommands::TxInfo { txid, raw } => {
            if raw {
                let tx_info = provider.get_tx_info(&txid).await?;
                let json_value = serde_json::to_value(&tx_info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let tx_info = provider.get_tx_info(&txid).await?;
                print_tx_info(&tx_info);
            }
        }
    }
    Ok(())
}

async fn execute_protorunes_command(
    provider: &dyn DeezelProvider,
    command: Protorunes,
) -> anyhow::Result<()> {
    match command {
        Protorunes::ByAddress {
            address,
            raw,
            block_tag,
            protocol_tag,
        } => {
            let resolved_address = provider.resolve_all_identifiers(&address).await?;
            let result = provider
                .protorunes_by_address(&resolved_address, block_tag, protocol_tag)
                .await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                pretty_print::print_protorune_wallet_response(&result);
            }
        }
        Protorunes::ByOutpoint {
            outpoint,
            raw,
            block_tag,
            protocol_tag,
        } => {
            let parts: Vec<&str> = outpoint.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid outpoint format. Expected txid:vout"));
            }
            let txid = parts[0].to_string();
            let vout = parts[1].parse::<u32>()?;
            let result = provider
                .protorunes_by_outpoint(&txid, vout, block_tag, protocol_tag)
                .await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                pretty_print::print_protorune_outpoint_response(&result);
            }
        }
    }
    Ok(())
}


async fn execute_brc20prog_command<T: System>(system: &mut T, command: commands::Brc20Prog, brc20_prog_rpc_url: Option<String>, jsonrpc_headers: Vec<(String, String)>, frbtc_address: Option<String>) -> Result<()> {
    use commands::Brc20Prog;
    use alkanes_cli_common::brc20_prog::{
        Brc20ProgExecutor, Brc20ProgExecuteParams, Brc20ProgDeployInscription,
        Brc20ProgCallInscription, parse_foundry_json, extract_deployment_bytecode,
        encode_function_call,
    };

    let provider = system.provider_mut();

    match command {
        Brc20Prog::DeployContract { foundry_json_path, from, change, fee_rate, raw, trace, mine, auto_confirm, no_activation, use_slipstream, use_rebar, rebar_tier, strategy, resume } => {
            let contract_data = parse_foundry_json(&foundry_json_path)?;
            let bytecode = extract_deployment_bytecode(&contract_data)?;

            let inscription = Brc20ProgDeployInscription::new(bytecode);
            let inscription_json = serde_json::to_string(&inscription)?;

            // Resolve address identifiers before creating params
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            // Parse strategy
            use alkanes_cli_common::brc20_prog::types::AntiFrontrunningStrategy;
            let parsed_strategy = if let Some(ref strat_str) = strategy {
                match strat_str.to_lowercase().as_str() {
                    "checklocktimeverify" | "cltv" => Some(AntiFrontrunningStrategy::CheckLockTimeVerify),
                    "cpfp" => Some(AntiFrontrunningStrategy::Cpfp),
                    "presign" => Some(AntiFrontrunningStrategy::Presign),
                    "rbf" => Some(AntiFrontrunningStrategy::Rbf),
                    _ => return Err(anyhow::anyhow!("Invalid strategy: {}. Valid options: checklocktimeverify, cpfp, presign, rbf", strat_str)),
                }
            } else {
                None
            };

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm: auto_confirm,
                use_activation: !no_activation, // Default is true (3-tx pattern), false with --no-activation
                use_slipstream,
                use_rebar,
                rebar_tier,
                strategy: parsed_strategy,
                resume_from_commit: resume,
                additional_outputs: None, // Not used for deploy
                mempool_indexer: false, // TODO: Add CLI flag when needed
            };

            let mut executor = Brc20ProgExecutor::new(provider);
            let result = executor.execute(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ Contract deployed successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                if let Some(ref activation_txid) = result.activation_txid {
                    println!("🔗 Activation TXID: {}", activation_txid);
                }
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                if let Some(activation_fee) = result.activation_fee {
                    println!("💰 Activation Fee: {} sats", activation_fee);
                }
            }
            Ok(())
        }
        Brc20Prog::Transact { address, signature, calldata, from, change, fee_rate, raw, trace, mine, auto_confirm, use_slipstream, use_rebar, rebar_tier, strategy, resume } => {
            let calldata_hex = encode_function_call(&signature, &calldata)?; // calldata.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())?;

            let inscription = Brc20ProgCallInscription::new(address, calldata_hex);
            let inscription_json = serde_json::to_string(&inscription)?;

            // Resolve address identifiers before creating params
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            // Parse strategy
            use alkanes_cli_common::brc20_prog::types::AntiFrontrunningStrategy;
            let parsed_strategy = if let Some(ref strat_str) = strategy {
                match strat_str.to_lowercase().as_str() {
                    "checklocktimeverify" | "cltv" => Some(AntiFrontrunningStrategy::CheckLockTimeVerify),
                    "cpfp" => Some(AntiFrontrunningStrategy::Cpfp),
                    "presign" => Some(AntiFrontrunningStrategy::Presign),
                    "rbf" => Some(AntiFrontrunningStrategy::Rbf),
                    _ => return Err(anyhow::anyhow!("Invalid strategy: {}. Valid options: checklocktimeverify, cpfp, presign, rbf", strat_str)),
                }
            } else {
                None
            };

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm: auto_confirm,
                use_activation: true, // Use 3-tx pattern (required for brc20-prog indexing)
                use_slipstream,
                use_rebar,
                rebar_tier,
                strategy: parsed_strategy,
                resume_from_commit: resume,
                additional_outputs: None, // Not used for transact
                mempool_indexer: false, // TODO: Add CLI flag when needed
            };

            let mut executor = Brc20ProgExecutor::new(provider);
            let result = executor.execute(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ Transaction executed successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            }
            Ok(())
        }
        Brc20Prog::WrapBtc { amount, from, change, fee_rate, raw, trace, mine, auto_confirm, use_slipstream, use_rebar, rebar_tier, resume } => {
            use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapParams};

            // Resolve address identifiers before creating params
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            let params = FrBtcWrapParams {
                amount,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
                use_slipstream,
                use_rebar,
                rebar_tier,
                resume_from_commit: resume,
            };

            let mut executor = FrBtcExecutor::new(provider);
            let result = executor.wrap(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ BTC wrapped to frBTC successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            }
            Ok(())
        }
        Brc20Prog::UnwrapBtc { amount, vout, to, from, change, fee_rate, raw, trace, mine, auto_confirm, use_slipstream, use_rebar, rebar_tier, resume } => {
            use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcUnwrapParams};

            // Resolve address identifiers
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            let resolved_to = provider.resolve_all_identifiers(&to).await?;

            let params = FrBtcUnwrapParams {
                amount,
                vout,
                recipient_address: resolved_to,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
                use_slipstream,
                use_rebar,
                rebar_tier,
                resume_from_commit: resume,
            };

            let mut executor = FrBtcExecutor::new(provider);
            let result = executor.unwrap(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ frBTC unwrap queued successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                println!("📬 BTC will be sent to {} by the subfrost operator", to);
            }
            Ok(())
        }
        Brc20Prog::WrapAndExecute { amount, script, from, change, fee_rate, raw, trace, mine, auto_confirm, use_slipstream, use_rebar, rebar_tier, resume } => {
            use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapAndExecuteParams};

            // Resolve address identifiers
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            let params = FrBtcWrapAndExecuteParams {
                amount,
                script_bytecode: script,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
                use_slipstream,
                use_rebar,
                rebar_tier,
                resume_from_commit: resume,
            };

            let mut executor = FrBtcExecutor::new(provider);
            let result = executor.wrap_and_execute(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ BTC wrapped and script executed!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            }
            Ok(())
        }
        Brc20Prog::WrapAndExecute2 { amount, target, signature, calldata, from, change, fee_rate, raw, trace, mine, auto_confirm, use_slipstream, use_rebar, rebar_tier, resume } => {
            use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapAndExecute2Params};

            // Resolve address identifiers
            let resolved_from = if let Some(from_addrs) = from {
                let mut resolved = Vec::new();
                for addr in from_addrs {
                    resolved.push(provider.resolve_all_identifiers(&addr).await?);
                }
                Some(resolved)
            } else {
                None
            };

            let resolved_change = if let Some(change_addr) = change {
                Some(provider.resolve_all_identifiers(&change_addr).await?)
            } else {
                None
            };

            let params = FrBtcWrapAndExecute2Params {
                amount,
                target_address: target,
                signature,
                calldata_args: calldata,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm,
                use_slipstream,
                use_rebar,
                rebar_tier,
                resume_from_commit: resume,
            };

            let mut executor = FrBtcExecutor::new(provider);
            let result = executor.wrap_and_execute2(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ BTC wrapped and contract called!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            }
            Ok(())
        }
        Brc20Prog::SignerAddress { raw } => {
            use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, get_frbtc_contract_address};

            let network = provider.get_network();
            let frbtc_address = get_frbtc_contract_address(network);

            let mut executor = FrBtcExecutor::new(provider);
            let signer_address = executor.get_signer_address().await?;

            if raw {
                let result = serde_json::json!({
                    "network": format!("{:?}", network),
                    "frbtc_contract": frbtc_address,
                    "signer_address": signer_address,
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🔑 FrBTC Signer Address");
                println!("   Network: {:?}", network);
                println!("   FrBTC Contract: {}", frbtc_address);
                println!("   Signer Address: {}", signer_address);
            }
            Ok(())
        }
        Brc20Prog::GetContractDeploys { address, raw } => {
            use alkanes_cli_common::traits::EsploraProvider;
            use alkanes_cli_common::brc20_prog::{pkscript_to_eth_address, compute_contract_address};
            
            // Resolve address identifier
            let resolved_address = provider.resolve_all_identifiers(&address).await?;
            
            // Get all transactions for this address
            let txs_json = provider.get_address_txs(&resolved_address).await?;
            let txs: Vec<serde_json::Value> = serde_json::from_value(txs_json)
                .map_err(|e| anyhow::anyhow!("Failed to parse transactions: {}", e))?;
            
            let mut deployments = Vec::new();
            
            for tx in txs {
                let txid = tx["txid"].as_str().unwrap_or("");
                
                // Try to get the transaction details to check for brc20-prog deploy
                if let Ok(tx_details) = provider.get_tx(txid).await {
                    // Check if this is a reveal transaction (has OP_RETURN with BRC20PROG)
                    let mut is_reveal = false;
                    if let Some(vout) = tx_details.get("vout").and_then(|v| v.as_array()) {
                        for output in vout.iter() {
                            if output.get("scriptpubkey_type").and_then(|v| v.as_str()) == Some("op_return") {
                                // Check if it contains "BRC20PROG" (hex: 425243323050524f47)
                                if let Some(script_hex) = output.get("scriptpubkey").and_then(|v| v.as_str()) {
                                    if script_hex.contains("425243323050524f47") {
                                        is_reveal = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    // If this is a reveal transaction, compute the contract address
                    if is_reveal {
                        // Get the input address (deployer) from vin[0]
                        let deployer_eth_address = if let Some(vin) = tx_details.get("vin").and_then(|v| v.as_array()) {
                            if let Some(input) = vin.get(0) {
                                // Get prevout scriptpubkey
                                if let Some(prevout) = input.get("prevout") {
                                    if let Some(scriptpubkey) = prevout.get("scriptpubkey").and_then(|v| v.as_str()) {
                                        pkscript_to_eth_address(scriptpubkey).ok()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        
                        // Compute contract address (nonce is typically 0 for first deployment)
                        let contract_address = if let Some(ref deployer) = deployer_eth_address {
                            compute_contract_address(deployer, 0).ok()
                        } else {
                            None
                        };
                        
                        deployments.push(serde_json::json!({
                            "txid": txid,
                            "block_height": tx.get("status").and_then(|s| s.get("block_height")),
                            "confirmed": tx.get("status").and_then(|s| s.get("confirmed")),
                            "deployer_eth_address": deployer_eth_address,
                            "contract_address": contract_address,
                        }));
                    }
                }
            }
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&deployments)?);
            } else {
                if deployments.is_empty() {
                    println!("No contract deployments found for address: {}", resolved_address);
                } else {
                    println!("Found {} contract deployment(s) for {}:\n", deployments.len(), resolved_address);
                    for (idx, deploy) in deployments.iter().enumerate() {
                        println!("{}. Reveal TXID: {}", idx + 1, deploy["txid"].as_str().unwrap_or("unknown"));
                        if let Some(height) = deploy.get("block_height") {
                            println!("   Block Height: {}", height);
                        }
                        if let Some(deployer) = deploy.get("deployer_eth_address").and_then(|v| v.as_str()) {
                            println!("   Deployer (ETH): {}", deployer);
                        }
                        if let Some(contract) = deploy.get("contract_address").and_then(|v| v.as_str()) {
                            println!("   Contract Address: {}", contract);
                        }
                        println!();
                    }
                    println!("💡 Use --brc20-prog-rpc-url with 'get-code' to verify the deployment");
                }
            }
            Ok(())
        }
        Brc20Prog::GetCode { address, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            // Get BRC20-Prog RPC URL from parameter
            let rpc_url = brc20_prog_rpc_url
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let code = client.eth_get_code(&address).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"code": code}))?);
            } else {
                if code == "0x" || code.is_empty() {
                    println!("No code at address: {}", address);
                } else {
                    println!("Contract bytecode at {}:", address);
                    println!("{}", code);
                    println!("\nBytecode length: {} bytes", (code.len() - 2) / 2);
                }
            }
            Ok(())
        }
        Brc20Prog::Call { to, data, from, block, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            use alkanes_cli_common::brc20_prog_rpc_types::EthCallParams;
            
            // Get BRC20-Prog RPC URL from parameter
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let params = EthCallParams { to, data, from, gas: None, gas_price: None, value: None };
            let result = client.eth_call(params, block.as_deref()).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"result": result}))?);
            } else {
                println!("Call result: {}", result);
            }
            Ok(())
        }
        Brc20Prog::GetBalance { address, block, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            // Get BRC20-Prog RPC URL from parameter
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let balance = client.eth_get_balance(&address, &block).await?;
            
            // Parse the hex balance
            let balance_hex = balance.trim_start_matches("0x");
            let balance_wei = u128::from_str_radix(balance_hex, 16)
                .unwrap_or(0);
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "balance": balance,
                    "balance_wei": balance_wei.to_string(),
                }))?);
            } else {
                // Convert wei to frBTC (assuming 18 decimals)
                let balance_btc = balance_wei as f64 / 1e18;
                println!("Address: {}", address);
                println!("Balance: {} wei", balance_wei);
                println!("Balance: {:.8} frBTC", balance_btc);
            }
            Ok(())
        }
        Brc20Prog::EstimateGas { to, data, from, block, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::{Brc20ProgRpcClient};
            use alkanes_cli_common::brc20_prog_rpc_types::EthCallParams;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let params = EthCallParams { to, data, from, gas: None, gas_price: None, value: None };
            let gas = client.eth_estimate_gas(params, block.as_deref()).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"gas": gas}))?);
            } else {
                let gas_hex = gas.trim_start_matches("0x");
                let gas_amount = u64::from_str_radix(gas_hex, 16).unwrap_or(0);
                println!("Estimated gas: {} ({gas})", gas_amount);
            }
            Ok(())
        }
        Brc20Prog::BlockNumber { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let block_num = client.eth_block_number().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"blockNumber": block_num}))?);
            } else {
                let num_hex = block_num.trim_start_matches("0x");
                let num = u64::from_str_radix(num_hex, 16).unwrap_or(0);
                println!("Block number: {} ({block_num})", num);
            }
            Ok(())
        }
        Brc20Prog::GetBlockByNumber { block, full, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let block_info = client.eth_get_block_by_number(&block, full).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&block_info)?);
            } else {
                println!("Block #{}", block_info.number);
                println!("  Hash: {}", block_info.hash);
                println!("  Parent: {}", block_info.parent_hash);
                println!("  Timestamp: {}", block_info.timestamp);
                println!("  Gas Used: {}", block_info.gas_used);
                println!("  Gas Limit: {}", block_info.gas_limit);
                println!("  Transactions: {}", block_info.transactions.len());
            }
            Ok(())
        }
        Brc20Prog::GetBlockByHash { hash, full, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let block_info = client.eth_get_block_by_hash(&hash, full).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&block_info)?);
            } else {
                println!("Block #{}", block_info.number);
                println!("  Hash: {}", block_info.hash);
                println!("  Parent: {}", block_info.parent_hash);
                println!("  Timestamp: {}", block_info.timestamp);
                println!("  Gas Used: {}", block_info.gas_used);
                println!("  Gas Limit: {}", block_info.gas_limit);
                println!("  Transactions: {}", block_info.transactions.len());
            }
            Ok(())
        }
        Brc20Prog::GetTransactionCount { address, block, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let nonce = client.eth_get_transaction_count(&address, &block).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"nonce": nonce}))?);
            } else {
                let nonce_hex = nonce.trim_start_matches("0x");
                let nonce_num = u64::from_str_radix(nonce_hex, 16).unwrap_or(0);
                println!("Transaction count (nonce): {} ({nonce})", nonce_num);
            }
            Ok(())
        }
        Brc20Prog::GetTransaction { hash, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let tx = client.eth_get_transaction_by_hash(&hash).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&tx)?);
            } else {
                if let Some(tx) = tx {
                    println!("Transaction {}", tx.hash);
                    println!("  From: {}", tx.from);
                    if let Some(to) = tx.to {
                        println!("  To: {}", to);
                    } else {
                        println!("  To: (contract creation)");
                    }
                    println!("  Block: #{}", tx.block_number);
                    println!("  Nonce: {}", tx.nonce);
                    println!("  Gas: {}", tx.gas);
                    println!("  Input: {}...", &tx.input[..tx.input.len().min(66)]);
                } else {
                    println!("Transaction not found");
                }
            }
            Ok(())
        }
        Brc20Prog::GetTransactionReceipt { hash, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let receipt = client.eth_get_transaction_receipt(&hash).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                if let Some(receipt) = receipt {
                    println!("Transaction Receipt");
                    println!("  TX Hash: {}", receipt.transaction_hash);
                    println!("  Block: #{}", receipt.block_number);
                    println!("  From: {}", receipt.from);
                    if let Some(to) = receipt.to {
                        println!("  To: {}", to);
                    }
                    if let Some(contract) = receipt.contract_address {
                        println!("  Contract Address: {}", contract);
                    }
                    println!("  Status: {}", if receipt.status == "0x1" { "Success" } else { "Failed" });
                    println!("  Gas Used: {}", receipt.gas_used);
                    println!("  Logs: {}", receipt.logs.len());
                } else {
                    println!("Receipt not found");
                }
            }
            Ok(())
        }
        Brc20Prog::GetStorageAt { address, position, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let value = client.eth_get_storage_at(&address, &position).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"value": value}))?);
            } else {
                println!("Storage at {} position {}:", address, position);
                println!("{}", value);
            }
            Ok(())
        }
        Brc20Prog::GetLogs { from_block, to_block, address, topics, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            use alkanes_cli_common::brc20_prog_rpc_types::GetLogsFilter;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            
            let topics_parsed = if let Some(topics_str) = topics {
                Some(serde_json::from_str(&topics_str)?)
            } else {
                None
            };
            
            let filter = GetLogsFilter {
                from_block,
                to_block,
                address: if address.is_empty() { None } else { Some(address) },
                topics: topics_parsed,
                block_hash: None,
            };
            
            let logs = client.eth_get_logs(filter).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&logs)?);
            } else {
                println!("Found {} log(s):", logs.len());
                for (i, log) in logs.iter().enumerate() {
                    println!("\n{}. Log", i + 1);
                    println!("   Address: {}", log.address);
                    println!("   Block: #{}", log.block_number);
                    println!("   TX: {}", log.transaction_hash);
                    println!("   Topics: {}", log.topics.len());
                    for (j, topic) in log.topics.iter().enumerate() {
                        println!("     [{}]: {}", j, topic);
                    }
                    println!("   Data: {}...", &log.data[..log.data.len().min(66)]);
                }
            }
            Ok(())
        }
        Brc20Prog::ChainId { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let chain_id = client.eth_chain_id().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"chainId": chain_id}))?);
            } else {
                let id_hex = chain_id.trim_start_matches("0x");
                let id_num = u64::from_str_radix(id_hex, 16).unwrap_or(0);
                println!("Chain ID: {} ({chain_id})", id_num);
            }
            Ok(())
        }
        Brc20Prog::GasPrice { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let gas_price = client.eth_gas_price().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"gasPrice": gas_price}))?);
            } else {
                println!("Gas price: {} wei", gas_price);
            }
            Ok(())
        }
        Brc20Prog::Version { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let version = client.brc20_version().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"version": version}))?);
            } else {
                println!("BRC20-Prog version: {}", version);
            }
            Ok(())
        }
        Brc20Prog::GetReceiptByInscription { inscription_id, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let receipt = client.brc20_get_tx_receipt_by_inscription_id(&inscription_id).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&receipt)?);
            } else {
                if let Some(receipt) = receipt {
                    println!("Transaction Receipt for inscription {}", inscription_id);
                    println!("  TX Hash: {}", receipt.transaction_hash);
                    println!("  Block: #{}", receipt.block_number);
                    println!("  Status: {}", if receipt.status == "0x1" { "Success" } else { "Failed" });
                    if let Some(contract) = receipt.contract_address {
                        println!("  Contract Address: {}", contract);
                    }
                } else {
                    println!("Receipt not found for inscription: {}", inscription_id);
                }
            }
            Ok(())
        }
        Brc20Prog::GetInscriptionByTx { tx_hash, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let inscription_id = client.brc20_get_inscription_id_by_tx_hash(&tx_hash).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&inscription_id)?);
            } else {
                if let Some(id) = inscription_id {
                    println!("Inscription ID: {}", id);
                } else {
                    println!("No inscription found for TX: {}", tx_hash);
                }
            }
            Ok(())
        }
        Brc20Prog::GetInscriptionByContract { address, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let inscription_id = client.brc20_get_inscription_id_by_contract_address(&address).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&inscription_id)?);
            } else {
                if let Some(id) = inscription_id {
                    println!("Inscription ID: {}", id);
                } else {
                    println!("No inscription found for contract: {}", address);
                }
            }
            Ok(())
        }
        Brc20Prog::Brc20Balance { pkscript, ticker, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let balance = client.brc20_balance(&pkscript, &ticker).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"balance": balance}))?);
            } else {
                println!("BRC20 Balance for {} ({}): {}", ticker, pkscript, balance);
            }
            Ok(())
        }
        Brc20Prog::TraceTransaction { hash, raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let trace = client.debug_trace_transaction(&hash).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&trace)?);
            } else {
                if let Some(trace) = trace {
                    println!("Transaction Trace for {}", hash);
                    println!("  Gas: {}", trace.gas);
                    println!("  Failed: {}", trace.failed);
                    println!("  Return Value: {}...", &trace.return_value[..trace.return_value.len().min(66)]);
                    println!("  Struct Logs: {} steps", trace.struct_logs.len());
                } else {
                    println!("Trace not found");
                }
            }
            Ok(())
        }
        Brc20Prog::TxpoolContent { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let content = client.txpool_content().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&content)?);
            } else {
                println!("Transaction Pool Content:");
                println!("  Pending: {}", serde_json::to_string_pretty(&content.pending)?);
                println!("  Queued: {}", serde_json::to_string_pretty(&content.queued)?);
            }
            Ok(())
        }
        Brc20Prog::ClientVersion { raw } => {
            use alkanes_cli_common::brc20_prog_rpc::Brc20ProgRpcClient;
            
            let rpc_url = brc20_prog_rpc_url.clone()
                .ok_or_else(|| anyhow::anyhow!("BRC20-Prog RPC URL not set. Use --brc20-prog-rpc-url flag"))?;
            
            let client = Brc20ProgRpcClient::with_headers(rpc_url, jsonrpc_headers.clone())?;
            let version = client.web3_client_version().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"version": version}))?);
            } else {
                println!("Client version: {}", version);
            }
            Ok(())
        }
        Brc20Prog::Unwrap { block_tag, raw, experimental_asm, experimental_sol } => {
            use alkanes_cli_common::unwrap::{MetaprotocolUnwrap, Brc20ProgUnwrap};
            use alkanes_cli_common::brc20_prog::{get_frbtc_address, get_signer_address};
            use alkanes_cli_common::traits::{JsonRpcProvider, EsploraProvider};

            // Create BRC20-Prog unwrap implementation with optional address override
            let brc20_impl = match frbtc_address.as_deref() {
                Some(addr) => Brc20ProgUnwrap::with_frbtc_address(addr),
                None => Brc20ProgUnwrap::new(),
            };

            // Get unfiltered unwraps from BRC20-Prog
            let confirmations_required = 6; // Require 6 confirmations for safety

            let all_unwraps = if experimental_sol {
                log::info!("🔬 Using experimental Solidity-compiled bytecode");
                let frbtc_addr = frbtc_address.as_deref();
                brc20_impl.get_pending_unwraps_experimental_sol(provider, confirmations_required, frbtc_addr, block_tag.as_deref()).await?
            } else if experimental_asm {
                log::info!("🚀 Using experimental ASM bytecode generator (100x faster!)");
                let frbtc_addr = frbtc_address.as_deref();
                brc20_impl.get_pending_unwraps_experimental_asm(provider, confirmations_required, frbtc_addr, block_tag.as_deref()).await?
            } else {
                brc20_impl.get_pending_unwraps(provider, confirmations_required).await?
            };
            
            log::info!("[BRC20-Prog Unwrap] Got {} unfiltered unwraps", all_unwraps.len());

            // Get FrBTC contract address and signer address for filtering
            // Use override if provided, otherwise use default for network
            let network = provider.get_network();
            let frbtc_addr = frbtc_address.as_deref()
                .unwrap_or_else(|| get_frbtc_address(network));
            let brc20_prog_rpc_url = provider.get_brc20_prog_rpc_url()
                .ok_or_else(|| anyhow::anyhow!("brc20_prog_rpc_url not configured"))?;

            // Get signer address (p2tr script_pubkey) from FrBTC contract
            let signer_script = get_signer_address(
                provider as &dyn JsonRpcProvider,
                &brc20_prog_rpc_url,
                frbtc_addr,
            ).await?;
            
            // Convert script_pubkey to taproot address
            let script_buf = bitcoin::ScriptBuf::from_bytes(signer_script.to_vec());
            let signer_address = bitcoin::Address::from_script(&script_buf, network)
                .map_err(|e| anyhow::anyhow!("Failed to convert script to address: {}", e))?
                .to_string();
            
            log::info!("[BRC20-Prog Unwrap] FrBTC signer address: {}", signer_address);
            
            // Get UTXOs at the FrBTC signer address (not wallet UTXOs!)
            let signer_utxos_json = provider.get_address_utxo(&signer_address).await?;
            let signer_utxos = signer_utxos_json.as_array()
                .ok_or_else(|| anyhow::anyhow!("Signer UTXOs response is not an array"))?;
            
            // Build UTXO set from signer address
            let utxo_set: std::collections::HashSet<String> = signer_utxos
                .iter()
                .filter_map(|utxo| {
                    let txid = utxo["txid"].as_str()?;
                    let vout = utxo["vout"].as_u64()?;
                    Some(format!("{}:{}", txid, vout))
                })
                .collect();
            
            log::info!("[BRC20-Prog Unwrap] Found {} UTXOs at signer address", utxo_set.len());
            
            // Filter unwraps by signer address UTXOs
            let result: Vec<_> = all_unwraps
                .into_iter()
                .filter(|u| utxo_set.contains(&format!("{}:{}", u.txid, u.vout)))
                .collect();
            
            log::info!("[BRC20-Prog Unwrap] Filtered to {} spendable unwraps", result.len());
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if result.is_empty() {
                    println!("✨ No pending BRC20-Prog unwraps found");
                    println!();
                    println!("Note: Results are filtered to only show unwraps with spendable UTXOs at the FrBTC signer address.");
                    println!("      Already fulfilled unwraps are automatically excluded.");
                } else {
                    println!("🔓 Pending BRC20-Prog Unwraps ({} total):", result.len());
                    println!();
                    println!("Note: Showing only unwraps with spendable UTXOs still available at the FrBTC signer address.");
                    println!("      Already fulfilled unwraps have been filtered out.");
                    println!();
                    
                    let total_amount: u64 = result.iter().map(|u| u.amount).sum();
                    println!("Total unwrap amount: {} sats ({:.8} BTC)", total_amount, total_amount as f64 / 100_000_000.0);
                    println!();
                    
                    for (i, unwrap) in result.iter().enumerate() {
                        println!("  {}. ⏳ Pending", i + 1);
                        println!("     Outpoint: {}:{}", unwrap.txid, unwrap.vout);
                        println!("     Amount:   {} sats ({:.8} BTC)", unwrap.amount, unwrap.amount as f64 / 100_000_000.0);
                        if let Some(ref addr) = unwrap.address {
                            println!("     To:       {}", addr);
                        }
                        println!();
                    }
                }
            }
            Ok(())
        }
    }
}

async fn execute_espo_command(
    provider: &dyn DeezelProvider,
    command: alkanes_cli_common::commands::EspoCommands,
) -> anyhow::Result<()> {
    use alkanes_cli_common::commands::EspoCommands;
    use alkanes_cli_common::traits::EspoProvider;

    match command {
        EspoCommands::Height { raw } => {
            let height = provider.get_espo_height().await?;
            if raw {
                println!("{}", height);
            } else {
                println!("ESPO Indexer Height: {}", height);
            }
        }
        EspoCommands::Balances { address, include_outpoints, raw } => {
            let result = provider.get_address_balances(&address, include_outpoints).await?;
            if raw {
                println!("{}", result);
            } else {
                println!("Alkanes Balances for {}:", address);
                if let Some(balances) = result.get("balances").and_then(|v| v.as_object()) {
                    for (alkane_id, balance) in balances {
                        println!("  {}: {}", alkane_id, balance);
                    }
                    if balances.is_empty() {
                        println!("  (no balances)");
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        EspoCommands::Outpoints { address, raw } => {
            let result = provider.get_address_outpoints(&address).await?;
            if raw {
                println!("{}", result);
            } else {
                println!("Outpoints with Alkanes for {}:", address);
                if let Some(outpoints) = result.get("outpoints").and_then(|v| v.as_array()) {
                    for outpoint in outpoints {
                        if let Some(op_str) = outpoint.as_str() {
                            println!("  {}", op_str);
                        } else {
                            println!("  {}", outpoint);
                        }
                    }
                    if outpoints.is_empty() {
                        println!("  (no outpoints)");
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        EspoCommands::Outpoint { outpoint, raw } => {
            let result = provider.get_outpoint_balances(&outpoint).await?;
            if raw {
                println!("{}", result);
            } else {
                println!("Alkanes at {}:", outpoint);
                if let Some(balances) = result.get("balances").and_then(|v| v.as_object()) {
                    for (alkane_id, balance) in balances {
                        println!("  {}: {}", alkane_id, balance);
                    }
                    if balances.is_empty() {
                        println!("  (no alkanes)");
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        EspoCommands::Holders { alkane_id, page, limit, raw } => {
            let result = provider.get_holders(&alkane_id, page, limit).await?;
            if raw {
                println!("{}", result);
            } else {
                println!("Holders of {} (page {}, limit {}):", alkane_id, page, limit);
                if let Some(holders) = result.get("holders").and_then(|v| v.as_array()) {
                    for holder in holders {
                        if let Some(obj) = holder.as_object() {
                            let addr = obj.get("address").and_then(|v| v.as_str()).unwrap_or("?");
                            let default_balance = serde_json::json!("?");
                            let balance = obj.get("balance").unwrap_or(&default_balance);
                            println!("  {}: {}", addr, balance);
                        } else {
                            println!("  {}", holder);
                        }
                    }
                    if holders.is_empty() {
                        println!("  (no holders)");
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        EspoCommands::HoldersCount { alkane_id, raw } => {
            let result = provider.get_holders_count(&alkane_id).await?;
            if raw {
                println!("{}", result);
            } else {
                if let Some(count) = result.get("count").and_then(|v| v.as_u64()) {
                    println!("Holder count for {}: {}", alkane_id, count);
                } else {
                    println!("Holder count for {}: {}", alkane_id, result);
                }
            }
        }
        EspoCommands::Keys { alkane_id, page, limit, raw } => {
            let result = provider.get_keys(&alkane_id, page, limit).await?;
            if raw {
                println!("{}", result);
            } else {
                println!("Storage keys for {} (page {}, limit {}):", alkane_id, page, limit);
                if let Some(keys) = result.get("keys").and_then(|v| v.as_array()) {
                    for key in keys {
                        if let Some(key_str) = key.as_str() {
                            println!("  {}", key_str);
                        } else {
                            println!("  {}", key);
                        }
                    }
                    if keys.is_empty() {
                        println!("  (no keys)");
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        EspoCommands::Ping => {
            let result = provider.ping().await?;
            println!("{}", result);
        }
        EspoCommands::AmmdataPing => {
            let result = provider.ammdata_ping().await?;
            println!("{}", result);
        }
        EspoCommands::Candles { pool, timeframe, side, limit, page, raw } => {
            let result = provider.get_candles(
                &pool,
                timeframe.as_deref(),
                side.as_deref(),
                limit,
                page,
            ).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Candles for pool {}:", pool);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        EspoCommands::Trades { pool, limit, page, side, filter_side, sort, dir, raw } => {
            let result = provider.get_trades(
                &pool,
                limit,
                page,
                side.as_deref(),
                filter_side.as_deref(),
                sort.as_deref(),
                dir.as_deref(),
            ).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Trades for pool {}:", pool);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        EspoCommands::Pools { limit, page, raw } => {
            let result = provider.get_pools(limit, page).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("All pools:");
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        EspoCommands::FindBestSwapPath {
            token_in,
            token_out,
            mode,
            amount_in,
            amount_out,
            amount_out_min,
            amount_in_max,
            available_in,
            fee_bps,
            max_hops,
            raw
        } => {
            let result = provider.find_best_swap_path(
                &token_in,
                &token_out,
                mode.as_deref(),
                amount_in.as_deref(),
                amount_out.as_deref(),
                amount_out_min.as_deref(),
                amount_in_max.as_deref(),
                available_in.as_deref(),
                fee_bps,
                max_hops,
            ).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Best swap path from {} to {}:", token_in, token_out);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        EspoCommands::GetBestMevSwap { token, fee_bps, max_hops, raw } => {
            let result = provider.get_best_mev_swap(&token, fee_bps, max_hops).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Best MEV swap for token {}:", token);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
    }
    Ok(())
}

