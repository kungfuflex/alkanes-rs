//! DEEZEL CLI - A thin wrapper around the deezel-sys library
//!
//! This crate is responsible for parsing command-line arguments and delegating
//! the actual work to the deezel-sys library. This keeps the CLI crate
//! lightweight and focused on its primary role as a user interface.

use anyhow::Result;
use clap::Parser;
use alkanes_cli_sys::{SystemAlkanes, SystemOrd};
use alkanes_cli_common::traits::*;
use futures::future::join_all;
use serde_json::json;

mod commands;
mod pretty_print;
use commands::{Alkanes, AlkanesExecute, Commands, DeezelCommands, MetashrewCommands, Protorunes, Runestone, WalletCommands};
use alkanes_cli_common::alkanes;
use pretty_print::*;


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = DeezelCommands::parse();

    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    // Handle keystore logic

    // Convert DeezelCommands to Args
    let alkanes_args = alkanes_cli_common::commands::Args::from(&args);

    // Validate RPC config (ensure only one backend is configured)
    alkanes_args.rpc_config.validate()?;

    // Create a new SystemAlkanes instance
    let mut system = SystemAlkanes::new(&alkanes_args).await?;

    // Execute the command
    execute_command(&mut system, args.command).await
}

async fn execute_command<T: System + SystemOrd + UtxoProvider>(system: &mut T, command: Commands) -> Result<()> {
    match command {
        Commands::Bitcoind(cmd) => system.execute_bitcoind_command(cmd.into()).await.map_err(|e| e.into()),
        Commands::Wallet(cmd) => execute_wallet_command(system, cmd).await,
        Commands::Alkanes(cmd) => execute_alkanes_command(system, cmd).await,
        Commands::Runestone(cmd) => execute_runestone_command(system, cmd).await,
        Commands::Protorunes(cmd) => execute_protorunes_command(system.provider(), cmd).await,
        Commands::Ord(cmd) => execute_ord_command(system.provider(), cmd.into()).await,
        Commands::Esplora(cmd) => execute_esplora_command(system.provider(), cmd.into()).await,
        Commands::Metashrew(cmd) => execute_metashrew_command(system.provider(), cmd).await,
        Commands::Brc20Prog(cmd) => execute_brc20prog_command(system, cmd).await,
    }
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
        WalletCommands::Send { address, amount, fee_rate, send_all, from, change_address, use_rebar, rebar_tier, auto_confirm } => {
            let params = alkanes_cli_common::traits::SendParams {
                address,
                amount,
                fee_rate,
                send_all,
                from,
                change_address,
                auto_confirm,
                use_rebar,
                rebar_tier,
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
        Alkanes::Simulate { contract_id, params, raw } => {
            let context: alkanes_cli_common::proto::alkanes::MessageContextParcel = if let Some(_p) = params {
                // TODO: Parse params - for now use default
                Default::default()
            } else {
                Default::default()
            };
            let result = system.provider().simulate(&contract_id, &context).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Simulation result: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        },
        Alkanes::Sequence { raw, .. } => {
            let result = system.provider().sequence().await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Sequence: {}", serde_json::to_string_pretty(&result)?);
            }
            Ok(())
        },
        Alkanes::Spendables { address, raw } => {
            let result = system.provider().spendables_by_address(&address).await?;
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
            let result = AlkanesProvider::get_balance(system.provider(), address.as_deref()).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_alkane_balances(&result);
            }
            Ok(())
        }
        Alkanes::WrapBtc { amount, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
            use alkanes_cli_common::alkanes::wrap_btc::{WrapBtcExecutor, WrapBtcParams};
            
            let params = WrapBtcParams {
                amount,
                from_addresses: from,
                change_address: change,
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
            let result = provider.get_address_info(&params).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("🏠 Address {}:\n{}", params, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxs { params, raw } => {
            let result = provider.get_address_txs(&params).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("📄 Transactions for address {}:\n{}", params, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxsChain { params, raw } => {
            let result = provider.get_address_txs_chain(&params, None).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("⛓️ Chain transactions for address {}:\n{}", params, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressTxsMempool { address, raw } => {
            let result = provider.get_address_txs_mempool(&address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("⏳ Mempool transactions for address {}:\n{}", address, serde_json::to_string_pretty(&result)?);
            }
        }
        alkanes_cli_common::commands::EsploraCommands::AddressUtxo { address, raw } => {
            let result = provider.get_address_utxo(&address).await?;
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
            if raw {
                let info = provider.get_ord_address_info(&address).await?;
                let json_value = serde_json::to_value(&info)?;
                if let Some(s) = json_value.as_str() {
                    println!("{s}");
                } else {
                    println!("{json_value}");
                }
            } else {
                let info = provider.get_ord_address_info(&address).await?;
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
            let result = provider
                .protorunes_by_address(&address, block_tag, protocol_tag)
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


async fn execute_brc20prog_command<T: System>(system: &mut T, command: commands::Brc20Prog) -> Result<()> {
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

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: from,
                change_address: change,
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
                println!("💰 Commit Fee: {} sats", result.commit_fee);
                println!("💰 Reveal Fee: {} sats", result.reveal_fee);
            }
            Ok(())
        }
        Brc20Prog::Transact { address, signature, calldata, from, change, fee_rate, raw, trace, mine, auto_confirm } => {
            let calldata_hex = encode_function_call(&signature, &calldata)?; // calldata.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())?;

            let inscription = Brc20ProgCallInscription::new(address, calldata_hex);
            let inscription_json = serde_json::to_string(&inscription)?;

            let params = Brc20ProgExecuteParams {
                inscription_content: inscription_json,
                from_addresses: from,
                change_address: change,
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

            let params = Brc20ProgWrapBtcParams {
                amount,
                target_address: target,
                calldata: calldata_bytes,
                from_addresses: from,
                change_address: change,
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
    }
}
