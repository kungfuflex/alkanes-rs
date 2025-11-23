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
use commands::{Alkanes, AlkanesExecute, Commands, DeezelCommands, MetashrewCommands, Protorunes, Runestone, WalletCommands, DataApiCommand};
use alkanes_cli_common::alkanes;
use pretty_print::*;

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

    // Convert DeezelCommands to Args
    let alkanes_args = alkanes_cli_common::commands::Args::from(&args);

    // Validate RPC config (ensure only one backend is configured)
    alkanes_args.rpc_config.validate()?;

    // Create a new SystemAlkanes instance
    let mut system = SystemAlkanes::new(&alkanes_args).await?;

    // Set default brc20-prog RPC URL for signet if not provided
    let brc20_prog_rpc_url = alkanes_args.brc20_prog_rpc_url.clone().or_else(|| {
        if &alkanes_args.rpc_config.provider == "signet" {
            Some("https://signet-api.ordinalsbot.com/brc20/rpc".to_string())
        } else {
            None
        }
    });

    // Execute other commands
    execute_command(&mut system, args.command, brc20_prog_rpc_url, args.sandshrew_rpc_url.clone()).await
}

async fn execute_command<T: System + SystemOrd + UtxoProvider>(system: &mut T, command: Commands, brc20_prog_rpc_url: Option<String>, sandshrew_rpc_url: Option<String>) -> Result<()> {
    match command {
        Commands::Bitcoind(cmd) => system.execute_bitcoind_command(cmd.into()).await.map_err(|e| e.into()),
        Commands::Wallet(cmd) => execute_wallet_command(system, cmd).await,
        Commands::Alkanes(cmd) => execute_alkanes_command(system, cmd).await,
        Commands::Runestone(cmd) => execute_runestone_command(system, cmd).await,
        Commands::Protorunes(cmd) => execute_protorunes_command(system.provider(), cmd).await,
        Commands::Ord(cmd) => execute_ord_command(system.provider(), cmd.into()).await,
        Commands::Esplora(cmd) => execute_esplora_command(system.provider(), cmd.into()).await,
        Commands::Metashrew(cmd) => execute_metashrew_command(system.provider(), cmd).await,
        Commands::Sandshrew(command) => execute_sandshrew_command(system.provider(), command, sandshrew_rpc_url).await,
        Commands::Brc20Prog(cmd) => execute_brc20prog_command(system, cmd, brc20_prog_rpc_url).await,
        Commands::Dataapi(_) => {
            // Dataapi is handled in main() because it doesn't need the System trait
            unreachable!("Dataapi commands should be handled in main()")
        }
    }
}

async fn execute_dataapi_command(args: &DeezelCommands, command: DataApiCommand) -> Result<()> {
    use alkanes_cli_common::dataapi::DataApiClient;
    
    // Determine the data API URL based on --data-api flag or provider network
    let api_url = if let Some(ref url) = args.data_api {
        url.clone()
    } else {
        match args.provider.as_str() {
            "mainnet" => "https://mainnet-api.oyl.gg".to_string(),
            "regtest" | "signet" | "testnet" | _ => "http://localhost:4000".to_string(),
        }
    };
    
    let client = DataApiClient::new(api_url);
    
    match command {
        DataApiCommand::Health => {
            let result = alkanes_cli_common::dataapi::commands::execute_dataapi_health(&client).await?;
            println!("{}", result);
        }
        DataApiCommand::GetBitcoinPrice { raw } => {
            let price = client.get_bitcoin_price().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&price)?);
            } else {
                use alkanes_cli_sys::pretty_print::print_bitcoin_price;
                print_bitcoin_price(&price);
            }
        }
        DataApiCommand::GetAlkanes { limit, offset, sort_by, order, search, raw } => {
            let response = client.get_alkanes(limit, offset, sort_by, order, search).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                use alkanes_cli_sys::pretty_print::print_alkanes_response;
                print_alkanes_response(&response);
            }
        }
        DataApiCommand::GetAlkanesByAddress { address, raw } => {
            let tokens = client.get_alkanes_by_address(&address).await?;
            println!("{}", serde_json::to_string_pretty(&tokens)?);
        }
        DataApiCommand::GetAlkaneDetails { id, raw } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            let alkane_id = parse_alkane_id(&id)?;
            let token = client.get_alkane_details(&alkane_id).await?;
            println!("{}", serde_json::to_string_pretty(&token)?);
        }
        DataApiCommand::GetPools { factory, raw } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            let factory_id = parse_alkane_id(&factory)?;
            let pools = client.get_pools(&factory_id).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&pools)?);
            } else {
                use alkanes_cli_sys::pretty_print::print_pools_response;
                print_pools_response(&pools);
            }
        }
        DataApiCommand::GetPoolById { id, raw } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            let pool_id = parse_alkane_id(&id)?;
            let pool = client.get_pool_by_id(&pool_id).await?;
            println!("{}", serde_json::to_string_pretty(&pool)?);
        }
        DataApiCommand::GetPoolHistory { pool_id, category, limit, offset, raw } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            use alkanes_cli_common::dataapi::HistoryTransaction;
            
            let pool_alkane_id = parse_alkane_id(&pool_id)?;
            let history = client.get_pool_history(&pool_alkane_id, category, limit, offset).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                // Extract swaps, mints, burns from transactions
                let mut swaps = Vec::new();
                let mut mints = Vec::new();
                let mut burns = Vec::new();
                
                for tx in history.transactions {
                    match tx {
                        HistoryTransaction::Swap(swap) => swaps.push(swap),
                        HistoryTransaction::Mint(mint) => mints.push(mint),
                        HistoryTransaction::Burn(burn) => burns.push(burn),
                        _ => {},
                    }
                }
                
                let pool_history = alkanes_cli_common::dataapi::PoolHistoryResponse {
                    swaps,
                    mints,
                    burns,
                };
                use alkanes_cli_sys::pretty_print::print_pool_history;
                print_pool_history(&pool_history);
            }
        }
        DataApiCommand::GetSwapHistory { pool_id, limit, offset, raw } => {
            use alkanes_cli_common::dataapi::commands::parse_alkane_id;
            
            let pool_alkane_id = if let Some(ref id_str) = pool_id {
                Some(parse_alkane_id(id_str)?)
            } else {
                None
            };
            
            let history = client.get_swap_history(pool_alkane_id.as_ref(), limit, offset).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                use alkanes_cli_sys::pretty_print::print_swap_history;
                print_swap_history(&history.swaps);
            }
        }
        DataApiCommand::GetMarketChart { days, raw } => {
            let chart = client.get_bitcoin_market_chart(&days).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&chart)?);
            } else {
                use alkanes_cli_sys::pretty_print::print_market_chart;
                print_market_chart(&chart);
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
            };
            let txid = system.provider_mut().send(params).await?;
            println!("Transaction sent: {txid}");
        }
        WalletCommands::Balance { addresses, raw } => {
            let balance = WalletProvider::get_balance(system.provider(), addresses).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&balance)?);
            } else {
                println!("Confirmed: {}", balance.confirmed);
                println!("Pending:   {}", balance.pending);
            }
        }
        WalletCommands::History { count, address, raw } => {
            let history = system.provider().get_history(count, address).await?;
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
            let mut state = executor.execute(params.clone()).await?;

            loop {
                state = match state {
                    alkanes::types::ExecutionState::ReadyToSign(s) => {
                        let result = executor.resume_execution(s, &params).await?;
                        println!("\n✅ Alkanes execution completed successfully!");
                        println!("🔗 Reveal TXID: {}", result.reveal_txid);
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
                        break;
                    },
                    alkanes::types::ExecutionState::ReadyToSignCommit(s) => {
                        executor.resume_commit_execution(s).await?
                    },
                    alkanes::types::ExecutionState::ReadyToSignReveal(s) => {
                        let result = executor.resume_reveal_execution(s).await?;
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
                        break;
                    },
                    alkanes::types::ExecutionState::Complete(result) => {
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
                        break;
                    }
                };
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
            raw 
        } => {
            use alkanes_cli_common::proto::alkanes::{MessageContextParcel, AlkaneTransfer, AlkaneId, Uint128};
            use alkanes_cli_common::traits::MetashrewRpcProvider;
            use prost::Message;
            use alkanes_support::envelope::RawEnvelope;
            use bitcoin::{Transaction as BtcTransaction, TxIn, TxOut, OutPoint, Sequence, Amount, Address};
            use bitcoin::transaction::Version;
            
            // Parse alkane_id (format: block:tx:calldata_opcode, e.g., 4:65522:3)
            let parts: Vec<&str> = alkane_id.split(':').collect();
            if parts.len() < 2 {
                return Err(anyhow::anyhow!("Invalid alkane_id format. Expected block:tx or block:tx:calldata_opcode"));
            }
            
            let target_block: u64 = parts[0].parse()?;
            let target_tx: u64 = parts[1].parse()?;
            let calldata_opcode: u64 = if parts.len() >= 3 {
                parts[2].parse()?
            } else {
                0
            };
            
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
            
            // Build calldata: target_block, target_tx, calldata_opcode
            let mut calldata = Vec::new();
            leb128::write::unsigned(&mut calldata, target_block).unwrap();
            leb128::write::unsigned(&mut calldata, target_tx).unwrap();
            leb128::write::unsigned(&mut calldata, calldata_opcode).unwrap();
            
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
            log::debug!("Simulating alkane {}:{} with opcode {}", target_block, target_tx, calldata_opcode);
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
            // The result is a proto::alkanes::Trace which contains the trace
            if let Some(alkanes_trace) = result.trace {
                // Convert via protobuf encoding/decoding
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
                println!("No trace data found for block: {}", height);
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
                } else {
                    println!("🔓 Pending Unwraps ({} total):", result.len());
                    println!();
                    for (i, unwrap) in result.iter().enumerate() {
                        let status = if unwrap.fulfilled { "✅ Fulfilled" } else { "⏳ Pending" };
                        println!("  {}. {}", i + 1, status);
                        println!("     Outpoint: {}:{}", unwrap.txid, unwrap.vout);
                        println!("     Amount:   {} sats", unwrap.amount);
                        if let Some(ref addr) = unwrap.address {
                            println!("     Address:  {}", addr);
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
        Alkanes::GetAllPools { factory_id, raw } => {
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
            
            // Get all pools
            let result = amm_manager.get_all_pools(&factory, provider).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🏊 Found {} pool(s) from factory {}:{}", result.count, block, tx);
                println!();
                for (idx, pool) in result.pools.iter().enumerate() {
                    println!("  {}. Pool {}:{}", idx + 1, pool.block, pool.tx);
                }
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
            // Parse pool_id to AlkaneId
            let parts: Vec<&str> = pool_id.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow::anyhow!("Invalid pool_id format. Expected 'block:tx'"));
            }
            let block = parts[0].parse::<u64>()?;
            let tx = parts[1].parse::<u64>()?;
            let pool = alkanes::types::AlkaneId { block, tx };
            
            // Create AMM manager with a temporary executor
            use alkanes::execute::EnhancedAlkanesExecutor;
            let provider = system.provider();
            let mut provider_clone = provider.clone_box();
            let executor = std::sync::Arc::new(EnhancedAlkanesExecutor::new(&mut *provider_clone));
            let amm_manager = alkanes::amm::AmmManager::new(executor);
            
            // Get pool details
            let result = amm_manager.get_pool_details(&pool, provider).await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🏊 Pool Details for {}:{}", block, tx);
                println!();
                println!("  Name: {}", result.pool_name);
                println!("  Token0: {}:{}", result.token0.block, result.token0.tx);
                println!("    Amount: {}", result.token0_amount);
                println!("  Token1: {}:{}", result.token1.block, result.token1.tx);
                println!("    Amount: {}", result.token1_amount);
                println!("  LP Token Supply: {}", result.token_supply);
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
        Alkanes::Swap { path, input, minimum, expires, to, from, change, fee_rate, trace, factory } => {
            use alkanes_cli_common::alkanes::amm_cli::{execute_swap, SwapExecuteParams};
            use alkanes_cli_common::alkanes::types::AlkaneId;
            
            // Parse path (e.g., "2:0:32:0" for token0 -> token1)
            let path_str: Vec<&str> = path.split(':').collect();
            if path_str.len() % 2 != 0 {
                return Err(anyhow::anyhow!("Invalid path format. Expected BLOCK:TX:BLOCK:TX"));
            }
            
            let mut path_ids = Vec::new();
            for i in (0..path_str.len()).step_by(2) {
                path_ids.push(AlkaneId {
                    block: path_str[i].parse()?,
                    tx: path_str[i + 1].parse()?,
                });
            }
            
            let factory_id = {
                let parts: Vec<&str> = factory.split(':').collect();
                AlkaneId {
                    block: parts[0].parse()?,
                    tx: parts[1].parse()?,
                }
            };
            
            // Get current height for expiry if not provided
            let provider = system.provider_mut();
            let current_height = provider.get_height().await?;
            let expires_block = expires.unwrap_or(current_height + 10000);
            
            let params = SwapExecuteParams {
                factory_id,
                path: path_ids,
                input_amount: input.clone(),
                minimum_output: minimum.clone(),
                expires: expires_block,
                to_address: to.clone(),
                from_address: from.clone(),
                change_address: change.clone(),
                fee_rate: fee_rate.clone(),
                trace: trace.clone(),
                auto_confirm: false, // TODO: Add --auto-confirm flag to Swap command
            };
            
            let txid = execute_swap(provider, params).await?;
            println!("Transaction ID: {}", txid);
            Ok(())
        }
    }
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
    })
}

async fn execute_runestone_command<T: System>(system: &mut T, command: Runestone) -> Result<()> {
    match command {
        Runestone::Analyze { txid, raw } => {
            let tx_hex = system.provider().get_transaction_hex(&txid).await?;
            let tx_bytes = hex::decode(tx_hex)?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx)?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                alkanes_cli_common::runestone_enhanced::print_human_readable_runestone(&tx, &result);
            }
        }
        Runestone::Trace { txid, raw } => {
            use alkanes_cli_common::traits::AlkanesProvider;
            use prost::Message;
            
            // Get and analyze transaction
            let tx_hex = system.provider().get_transaction_hex(&txid).await?;
            let tx_bytes = hex::decode(tx_hex)?;
            let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)?;
            let result = alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&tx)?;
            
            // Print transaction structure
            if !raw {
                println!("🔍 ═══════════════════════════════════════════════════════════════");
                println!("🧪           RUNESTONE TRANSACTION TRACE ANALYSIS             🧪");
                println!("🔍 ═══════════════════════════════════════════════════════════════\n");
                println!("📝 Transaction ID: {}\n", txid);
                alkanes_cli_common::runestone_enhanced::print_human_readable_runestone(&tx, &result);
                println!("\n🔍 ═══════════════════════════════════════════════════════════════");
                println!("🧪                   PROTOSTONE TRACES                        🧪");
                println!("🔍 ═══════════════════════════════════════════════════════════════\n");
            }
            
            // Extract number of protostones
            let num_protostones = if let Some(protostones) = result.get("protostones").and_then(|p| p.as_array()) {
                protostones.len()
            } else {
                0
            };
            
            if num_protostones == 0 {
                if raw {
                    println!("{{\"transaction\": {}, \"traces\": []}}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("📭 No protostones found in this transaction.\n");
                }
                return Ok(());
            }
            
            // Calculate virtual vout indices and trace each protostone
            // Protostones are indexed starting at tx.output.len() + 1
            let base_vout = tx.output.len() as u32 + 1;
            let mut all_traces = Vec::new();
            
            for i in 0..num_protostones {
                let vout = base_vout + i as u32;
                let outpoint = format!("{}:{}", txid, vout);
                
                if !raw {
                    println!("📊 Protostone #{} (virtual vout {}):", i + 1, vout);
                    println!("   Outpoint: {}\n", outpoint);
                }
                
                match system.provider().trace(&outpoint).await {
                    Ok(trace_pb) => {
                        if let Some(alkanes_trace) = trace_pb.trace {
                            match alkanes_support::trace::Trace::try_from(
                                Message::encode_to_vec(&alkanes_trace)
                            ) {
                                Ok(trace) => {
                                    if raw {
                                        let json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                                        all_traces.push(json);
                                    } else {
                                        let pretty = alkanes_cli_common::alkanes::trace::format_trace_pretty(&trace);
                                        println!("{}\n", pretty);
                                    }
                                }
                                Err(e) => {
                                    if raw {
                                        all_traces.push(serde_json::json!({
                                            "error": format!("Failed to decode trace: {}", e),
                                            "events": []
                                        }));
                                    } else {
                                        println!("   ❌ Error: Failed to decode trace: {}\n", e);
                                    }
                                }
                            }
                        } else {
                            if raw {
                                all_traces.push(serde_json::json!({"events": []}));
                            } else {
                                println!("   ⚠️ No trace data found.\n");
                            }
                        }
                    }
                    Err(e) => {
                        if raw {
                            all_traces.push(serde_json::json!({
                                "error": format!("Failed to trace: {}", e),
                                "events": []
                            }));
                        } else {
                            println!("   ❌ Error: {}\n", e);
                        }
                    }
                }
            }
            
            if raw {
                let output = serde_json::json!({
                    "transaction": result,
                    "traces": all_traces
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("🎯 ═══════════════════════════════════════════════════════════════");
                println!("✨                      TRACE COMPLETE                         ✨");
                println!("🎯 ═══════════════════════════════════════════════════════════════");
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
                                match alkanes_cli_common::runestone_enhanced::format_runestone_with_decoded_messages(&transaction) {
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


async fn execute_brc20prog_command<T: System>(system: &mut T, command: commands::Brc20Prog, brc20_prog_rpc_url: Option<String>) -> Result<()> {
    use commands::Brc20Prog;
    use alkanes_cli_common::brc20_prog::{
        Brc20ProgExecutor, Brc20ProgExecuteParams, Brc20ProgDeployInscription,
        Brc20ProgCallInscription, parse_foundry_json, extract_deployment_bytecode,
        encode_function_call,
    };

    let provider = system.provider_mut();

    match command {
        Brc20Prog::DeployContract { foundry_json_path, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
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

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm: auto_confirm,
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
        Brc20Prog::Transact { address, signature, calldata, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
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

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm: auto_confirm,
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
        Brc20Prog::WrapBtc { amount, target, signature, calldata, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
            use alkanes_cli_common::brc20_prog::wrap_btc::{Brc20ProgWrapBtcExecutor, Brc20ProgWrapBtcParams};

            
            let calldata_hex = encode_function_call(&signature, &calldata)?;
            let calldata_bytes = hex::decode(calldata_hex.trim_start_matches("0x"))?;

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

            let params = Brc20ProgWrapBtcParams {
                amount,
                target_address: target,
                calldata: calldata_bytes,
                from_addresses: resolved_from,
                change_address: resolved_change,
                fee_rate,
                raw_output: raw,
                trace_enabled: trace,
                mine_enabled: mine,
                auto_confirm: auto_confirm,
            };

            let mut executor = Brc20ProgWrapBtcExecutor::new(provider);
            let result = executor.wrap_btc(params).await?;

            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("✅ BTC wrapped and locked successfully!");
                println!("🔗 Commit TXID: {}", result.commit_txid);
                println!("🔗 Reveal TXID: {}", result.reveal_txid);
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
                println!("🎉 frBTC minted and locked in BRC20 vault!");
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
            
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
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
            
            let client = Brc20ProgRpcClient::new(rpc_url)?;
            let version = client.web3_client_version().await?;
            
            if raw {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"version": version}))?);
            } else {
                println!("Client version: {}", version);
            }
            Ok(())
        }
    }
}

async fn execute_sandshrew_command(
    provider: &dyn DeezelProvider,
    command: crate::commands::SandshrewCommands,
    rpc_url: Option<String>,
) -> anyhow::Result<()> {
    use crate::commands::SandshrewCommands;
    use sha2::{Digest, Sha256};
    use std::fs;

    let rpc_url = rpc_url.ok_or_else(|| anyhow::anyhow!("Sandshrew RPC URL not set. Use --sandshrew-rpc-url"))?;

    match command {
        SandshrewCommands::Evalscript { script, args, raw } => {
            let script_content = fs::read_to_string(&script)
                .map_err(|e| anyhow::anyhow!("Failed to read script file {}: {}", script, e))?;

            // Hash the script
            let mut hasher = Sha256::new();
            hasher.update(script_content.as_bytes());
            let script_hash = hex::encode(hasher.finalize());

            // Resolve args
            let mut resolved_args = Vec::new();
            for arg in args {
                match provider.resolve_all_identifiers(&arg).await {
                    Ok(resolved) => resolved_args.push(serde_json::Value::String(resolved)),
                    Err(_) => resolved_args.push(serde_json::Value::String(arg)),
                }
            }

            let client = reqwest::Client::new();

            // Try evalsaved
            let mut params = vec![serde_json::Value::String(script_hash.clone())];
            params.extend(resolved_args.clone());

            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "sandshrew_evalsaved",
                "params": params,
                "id": 1
            });

            let response = client.post(&rpc_url)
                .json(&request)
                .send()
                .await?;

            let response_json: serde_json::Value = response.json().await?;

            if let Some(error) = response_json.get("error") {
                // Check if error indicates script not found
                let error_msg = error.get("message").and_then(|s| s.as_str()).unwrap_or("");
                if error_msg.contains("Script not found") {
                    // Fallback to evalscript
                    if !raw {
                        println!("Script not cached (hash: {}), falling back to evalscript...", script_hash);
                    }

                    let mut params = vec![serde_json::Value::String(script_content)];
                    params.extend(resolved_args);

                    let request = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "sandshrew_evalscript",
                        "params": params,
                        "id": 1
                    });

                    let response = client.post(&rpc_url)
                        .json(&request)
                        .send()
                        .await?;
                    
                    let response_json: serde_json::Value = response.json().await?;
                    
                    if let Some(result) = response_json.get("result") {
                        println!("{}", serde_json::to_string_pretty(result)?);
                    } else if let Some(error) = response_json.get("error") {
                         return Err(anyhow::anyhow!("RPC error: {}", error));
                    }
                } else {
                    return Err(anyhow::anyhow!("RPC error: {}", error));
                }
            } else if let Some(result) = response_json.get("result") {
                println!("{}", serde_json::to_string_pretty(result)?);
            }
        }
    }
    Ok(())
}
