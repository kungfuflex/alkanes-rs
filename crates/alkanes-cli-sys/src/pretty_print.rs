//! # Pretty-printing for transaction and runestone analysis
//!
//! This module provides functions to display transaction and runestone
//! analysis results in a human-readable format for the CLI.

use alkanes_cli_common::alkanes::types::{
    AlkaneMetadata, AlkanesInspectResult, FuzzingResults, ReadyToSignRevealTx, ReadyToSignTx,
};
/// Pretty-prints the analysis for a transaction that is ready to be signed.
pub fn pretty_print_ready_to_sign(state: &ReadyToSignTx) {
    println!("Transaction Preview:");
    state.analysis.pretty_print();

    if let Some(inspection_result) = &state.inspection_result {
        pretty_print_inspection_preview(inspection_result);
    }
}

/// Pretty-prints the analysis for a reveal transaction that is ready to be signed.
pub fn pretty_print_reveal_analysis(state: &ReadyToSignRevealTx) {
    println!("Transaction Preview:");
    state.analysis.pretty_print();

    if let Some(inspection_result) = &state.inspection_result {
        pretty_print_inspection_preview(inspection_result);
    }
}

/// Pretty-prints a preview of the inspection/simulation results.
fn pretty_print_inspection_preview(result: &AlkanesInspectResult) {
    println!("\nSimulation Preview:");
    if let Some(metadata) = &result.metadata {
        println!("  Contract: {}", metadata.name);
        if let Some(fuzzing_results) = &result.fuzzing_results {
            if let Some(opcode_result) = fuzzing_results.opcode_results.first() {
                if let Some(method) = metadata
                    .methods
                    .iter()
                    .find(|m| m.opcode == opcode_result.opcode)
                {
                    println!("  Method: {}", method.name);
                }
            }
        }
    }
    if let Some(fuzzing_results) = &result.fuzzing_results {
        pretty_print_fuzzing_results(fuzzing_results, &result.alkane_id).unwrap();
    } else if let Some(metadata) = &result.metadata {
        pretty_print_metadata(metadata);
    }
}

/// Pretty-prints the result of an alkane inspection, mimicking the output of deezel-old.
pub fn pretty_print_inspection_result(result: &AlkanesInspectResult) -> anyhow::Result<()> {
    if let Some(codehash) = &result.codehash {
        println!("=== WASM CODEHASH ===");
        println!("📦 WASM size: {} bytes", result.bytecode_length);
        println!("🔐 SHA3 (Keccak256): 0x{codehash}");
        println!("=====================");
    }

    if let Some(metadata) = &result.metadata {
        pretty_print_metadata(metadata);
    } else if let Some(error) = &result.metadata_error {
        println!("=== ALKANE METADATA ===");
        println!("Note: Failed to extract metadata from __meta export");
        println!("Error: {error}");
        println!("========================");
    }

    if let Some(disassembly) = &result.disassembly {
        println!("=== WASM DISASSEMBLY (WAT) ===");
        println!("{disassembly}");
        println!("==============================");
    }

    if let Some(fuzzing_results) = &result.fuzzing_results {
        pretty_print_fuzzing_results(fuzzing_results, &result.alkane_id)?;
    }

    Ok(())
}

/// Pretty-prints the metadata of an alkane contract.
fn pretty_print_metadata(metadata: &AlkaneMetadata) {
    println!("=== ALKANE METADATA ===");
    println!("📦 Contract: {}", metadata.name);
    println!("🏷️  Version: {}", metadata.version);

    if let Some(desc) = &metadata.description {
        println!("📝 Description: {desc}");
    }

    if metadata.methods.is_empty() {
        println!("⚠️  No methods found");
    } else {
        println!("🔧 Methods ({}):", metadata.methods.len());

        let mut sorted_methods = metadata.methods.clone();
        sorted_methods.sort_by_key(|m| m.opcode);

        for (i, method) in sorted_methods.iter().enumerate() {
            let is_last = i == sorted_methods.len() - 1;
            let prefix = if is_last { "└──" } else { "├──" };

            let params_str = if method.params.is_empty() {
                "()".to_string()
            } else {
                format!("({})", method.params.join(", "))
            };

            println!("   {} 🎯 {} {}", prefix, method.name, params_str);

            let detail_prefix = if is_last { "      " } else { "   │  " };
            println!("{}├─ 🔢 Opcode: {}", detail_prefix, method.opcode);
            println!("{}└─ 📤 Returns: {}", detail_prefix, method.returns);

            if !is_last {
                println!("   │");
            }
        }
    }

    println!("========================");
}

/// Pretty-prints the results of fuzzing analysis.
fn pretty_print_fuzzing_results(
    fuzzing_results: &FuzzingResults,
    alkane_id: &alkanes_cli_common::alkanes::types::AlkaneId,
) -> anyhow::Result<()> {
    println!("\n=== FUZZING ANALYSIS ===");
    println!("Alkane: {}:{}", alkane_id.block, alkane_id.tx);
    println!();
    println!(
        "📊 Total opcodes tested: {}",
        fuzzing_results.total_opcodes_tested
    );
    if fuzzing_results.opcodes_filtered_out > 0 {
        println!(
            "🔍 Opcodes filtered out (undefined behavior): {}",
            fuzzing_results.opcodes_filtered_out
        );
    }
    println!(
        "✅ Successful executions: {}",
        fuzzing_results.successful_executions
    );
    println!("❌ Failed executions: {}", fuzzing_results.failed_executions);
    println!(
        "🎯 Implemented opcodes: {} total",
        fuzzing_results.implemented_opcodes.len()
    );

    if !fuzzing_results.implemented_opcodes.is_empty() {
        println!();
        println!("🔍 Implemented Opcodes:");
        let ranges = compress_opcode_ranges(&fuzzing_results.implemented_opcodes);
        println!("   📋 Opcodes: {ranges}");

        println!();
        println!("📊 Detailed Results for Implemented Opcodes:");
        for result in &fuzzing_results.opcode_results {
            let status = if result.success { "✅" } else { "❌" };
            println!(
                "   {} Opcode {}: return={:?}, time={:?}μs",
                status, result.opcode, result.return_value, result.execution_time_micros
            );

            let decoded_data = decode_data_bytevector(&result.return_data);
            println!("      📦 Data: {decoded_data}");

            if !result.host_calls.is_empty() {
                println!("      🔧 Host Calls ({}):", result.host_calls.len());
                for (i, call) in result.host_calls.iter().enumerate() {
                    let call_prefix = if i == result.host_calls.len() - 1 {
                        "└─"
                    } else {
                        "├─"
                    };
                    println!(
                        "         {} {}: {} -> {}",
                        call_prefix,
                        call.function_name,
                        call.parameters.join(", "),
                        call.result
                    );
                }
            }

            if let Some(error) = &result.error {
                println!("      ⚠️  Error: {error}");
            }
        }
    }

    println!("========================");
    Ok(())
}

/// Decodes a byte vector for display, checking for common patterns.
fn decode_data_bytevector(data: &[u8]) -> String {
    if data.is_empty() {
        return "Empty (0 bytes)".to_string();
    }

    let hex_part = if data.len() <= 32 {
        format!("Hex: {}", hex::encode(data))
    } else {
        format!(
            "Hex: {} (first 32 bytes of {})",
            hex::encode(&data[..32]),
            data.len()
        )
    };

    if data.len() >= 4 && data[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
        let message_bytes = &data[4..];
        if let Ok(utf8_string) = String::from_utf8(message_bytes.to_vec()) {
            let clean_string = utf8_string.trim_matches('\0').trim();
            if !clean_string.is_empty() && clean_string.is_ascii() {
                return format!("{hex_part} | Solidity Error: \"{clean_string}\"");
            }
        }
        return format!("{hex_part} | Solidity Error");
    }

    if let Ok(utf8_string) = String::from_utf8(data.to_vec()) {
        let clean_string = utf8_string.trim_matches('\0').trim();
        if !clean_string.is_empty() && clean_string.is_ascii() && clean_string.len() > 3 {
            return format!("{hex_part} | UTF-8: \"{clean_string}\"");
        }
    }

    if data.len() == 16 {
        let value = u128::from_le_bytes(data.try_into().unwrap_or([0; 16]));
        return format!("{hex_part} | u128: {value}");
    } else if data.len() == 8 {
        let value = u64::from_le_bytes(data.try_into().unwrap_or([0; 8]));
        return format!("{hex_part} | u64: {value}");
    } else if data.len() == 4 {
        let value = u32::from_le_bytes(data.try_into().unwrap_or([0; 4]));
        return format!("{hex_part} | u32: {value}");
    }

    hex_part
}

/// Compresses a list of opcodes into readable ranges.
fn compress_opcode_ranges(opcodes: &[u128]) -> String {
    if opcodes.is_empty() {
        return String::new();
    }

    let mut sorted_opcodes = opcodes.to_vec();
    sorted_opcodes.sort_unstable();

    let mut ranges = Vec::new();
    let mut start = sorted_opcodes[0];
    let mut end = sorted_opcodes[0];

    for &opcode in sorted_opcodes.iter().skip(1) {
        if opcode == end + 1 {
            end = opcode;
        } else {
            if start == end {
                ranges.push(start.to_string());
            } else {
                ranges.push(format!("{start}-{end}"));
            }
            start = opcode;
            end = opcode;
        }
    }

    if start == end {
        ranges.push(start.to_string());
    } else {
        ranges.push(format!("{start}-{end}"));
    }

    ranges.join(", ")
}

pub fn pretty_print_blockchain_info(info: &serde_json::Value) -> anyhow::Result<()> {
    println!("Blockchain Info:");
    if let Some(obj) = info.as_object() {
        for (key, value) in obj {
            println!("  {}: {}", key, value);
        }
    }
    Ok(())
}

pub fn pretty_print_network_info(info: &serde_json::Value) -> anyhow::Result<()> {
    println!("Network Info:");
    if let Some(obj) = info.as_object() {
        for (key, value) in obj {
            println!("  {}: {}", key, value);
        }
    }
    Ok(())
}
// Pretty printing for DataAPI responses (added without cfg guards)
use alkanes_cli_common::dataapi::{AlkanesResponse, Pool, BitcoinPrice, MarketChart, PoolHistoryResponse, PoolSwap};
use colored::Colorize;

pub fn print_alkanes_response(response: &AlkanesResponse) {
    
    println!("\n{}", "📊 Alkanes Tokens".bold().cyan());
    println!("{}", "═".repeat(80).cyan());
    
    if response.tokens.is_empty() {
        println!("  {}", "No tokens found".yellow());
        println!();
        return;
    }
    
    for (idx, token) in response.tokens.iter().enumerate() {
        println!("\n{} {} {}", 
                 format!("{}.", idx + 1).bold(),
                 "🪙".bold(),
                 format!("{}:{}", token.id.block, token.id.tx).bright_green());
        
        if let Some(ref symbol) = token.symbol {
            if let Some(ref name) = token.name {
                println!("   {} {} ({})", "Token:".bold(), name.cyan(), symbol.yellow());
            } else {
                println!("   {} {}", "Symbol:".bold(), symbol.yellow());
            }
        } else if let Some(ref name) = token.name {
            println!("   {} {}", "Name:".bold(), name.cyan());
        }
        
        if let Some(decimals) = token.decimals {
            println!("   {} {}", "Decimals:".bold(), decimals);
        }
        
        if let Some(ref balance) = token.balance {
            println!("   {} {}", "Balance:".bold(), balance.bright_white());
        }
        
        if let Some(price_usd) = token.price_usd {
            println!("   {} ${:.4}", "Price USD:".bold(), price_usd);
        }
        
        if let Some(price_sat) = token.price_in_satoshi {
            println!("   {} {} sats", "Price BTC:".bold(), price_sat);
        }
    }
    
    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {}", "Total:".bold(), response.total);
    println!();
}

pub fn print_pools_response(pools: &[Pool]) {
    
    println!("\n{}", "🏊 Liquidity Pools".bold().cyan());
    println!("{}", "═".repeat(80).cyan());
    
    if pools.is_empty() {
        println!("  {}", "No pools found".yellow());
        println!();
        return;
    }
    
    for (idx, pool) in pools.iter().enumerate() {
        println!("\n{} {} {}", 
                 format!("{}.", idx + 1).bold(),
                 "💧".bold(),
                 pool.pool_name.bright_cyan().bold());
        
        println!("   {} {}:{}", "Pool ID:".bold(), 
                 pool.pool_block_id.green(), pool.pool_tx_id.green());
        println!("   {} {}:{} → {}:{}", "Pair:".bold(),
                 pool.token0_block_id.yellow(), pool.token0_tx_id.yellow(),
                 pool.token1_block_id.yellow(), pool.token1_tx_id.yellow());
        
        if let Some(ref amount0) = pool.token0_amount {
            if let Some(ref amount1) = pool.token1_amount {
                println!("   {} {} × {}", "Reserves:".bold(), 
                         amount0.bright_white(), amount1.bright_white());
            }
        }
        
        if let Some(ref supply) = pool.token_supply {
            println!("   {} {}", "LP Supply:".bold(), supply.bright_white());
        }
        
        if let Some(ref creator) = pool.creator_address {
            println!("   {} {}", "Creator:".bold(), creator.dimmed());
        }
    }
    
    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {} pools", "Total:".bold(), pools.len());
    println!();
}

pub fn print_bitcoin_price(price: &BitcoinPrice) {
    
    println!("\n{}", "₿ Bitcoin Price".bold().yellow());
    println!("{}", "═".repeat(50).yellow());
    println!("  {} ${:.2}", "USD:".bold(), price.usd);
    println!("{}", "─".repeat(50).yellow());
    println!();
}

pub fn print_market_chart(chart: &MarketChart) {
    
    println!("\n{}", "📈 Bitcoin Market Chart".bold().cyan());
    println!("{}", "═".repeat(80).cyan());
    
    if chart.prices.is_empty() {
        println!("  {}", "No data available".yellow());
        println!();
        return;
    }
    
    println!("\n{}", "  Prices:".bold());
    for (idx, price_point) in chart.prices.iter().enumerate().take(10) {
        if price_point.len() >= 2 {
            let timestamp = price_point[0] as i64 / 1000;
            let price = price_point[1];
            let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            println!("    {} {} - ${:.2}", 
                     format!("{}.", idx + 1).dimmed(),
                     datetime.bright_white(),
                     price);
        }
    }
    
    if chart.prices.len() > 10 {
        println!("    {} ... and {} more", "...".dimmed(), chart.prices.len() - 10);
    }
    
    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {} data points", "Total:".bold(), chart.prices.len());
    println!();
}

pub fn print_pool_history(history: &PoolHistoryResponse) {
    
    println!("\n{}", "📜 Pool History".bold().cyan());
    println!("{}", "═".repeat(80).cyan());
    
    let total_events = history.swaps.len() + history.mints.len() + history.burns.len();
    
    if total_events == 0 {
        println!("  {}", "No history found".yellow());
        println!();
        return;
    }
    
    // Print swaps
    if !history.swaps.is_empty() {
        println!("\n{} {} Swaps", "💱".bold(), history.swaps.len());
        for (idx, swap) in history.swaps.iter().enumerate().take(5) {
            println!("\n   {} Swap #{}", format!("{}.", idx + 1).bold(), swap.id.dimmed());
            println!("      {} {}:{} → {}:{}", "Pair:".bold(),
                     swap.sold_token_block_id.yellow(), swap.sold_token_tx_id.yellow(),
                     swap.bought_token_block_id.green(), swap.bought_token_tx_id.green());
            println!("      {} {:.4} → {:.4}", "Amount:".bold(), 
                     swap.sold_amount, swap.bought_amount);
            println!("      {} {} {}", "Status:".bold(),
                     if swap.successful { "✅".green() } else { "❌".red() },
                     if swap.successful { "Success" } else { "Failed" });
        }
        if history.swaps.len() > 5 {
            println!("   {} ... and {} more swaps", "...".dimmed(), history.swaps.len() - 5);
        }
    }
    
    // Print mints
    if !history.mints.is_empty() {
        println!("\n{} {} Mints", "➕".bold(), history.mints.len());
        for (idx, mint) in history.mints.iter().enumerate().take(5) {
            println!("\n   {} Mint #{}", format!("{}.", idx + 1).bold(), mint.id.dimmed());
            println!("      {} {}", "LP Tokens:".bold(), mint.lp_token_amount.bright_white());
            println!("      {} {} × {}", "Deposited:".bold(),
                     mint.token0_amount.yellow(), mint.token1_amount.yellow());
            println!("      {} {} {}", "Status:".bold(),
                     if mint.successful { "✅".green() } else { "❌".red() },
                     if mint.successful { "Success" } else { "Failed" });
        }
        if history.mints.len() > 5 {
            println!("   {} ... and {} more mints", "...".dimmed(), history.mints.len() - 5);
        }
    }
    
    // Print burns
    if !history.burns.is_empty() {
        println!("\n{} {} Burns", "➖".bold(), history.burns.len());
        for (idx, burn) in history.burns.iter().enumerate().take(5) {
            println!("\n   {} Burn #{}", format!("{}.", idx + 1).bold(), burn.id.dimmed());
            println!("      {} {}", "LP Tokens:".bold(), burn.lp_token_amount.bright_white());
            println!("      {} {} × {}", "Withdrawn:".bold(),
                     burn.token0_amount.yellow(), burn.token1_amount.yellow());
            println!("      {} {} {}", "Status:".bold(),
                     if burn.successful { "✅".green() } else { "❌".red() },
                     if burn.successful { "Success" } else { "Failed" });
        }
        if history.burns.len() > 5 {
            println!("   {} ... and {} more burns", "...".dimmed(), history.burns.len() - 5);
        }
    }
    
    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {} total events ({} swaps, {} mints, {} burns)", 
             "Total:".bold(), total_events, history.swaps.len(), history.mints.len(), history.burns.len());
    println!();
}

pub fn print_swap_history(swaps: &[PoolSwap]) {
    
    println!("\n{}", "💱 Swap History".bold().cyan());
    println!("{}", "═".repeat(80).cyan());
    
    if swaps.is_empty() {
        println!("  {}", "No swaps found".yellow());
        println!();
        return;
    }
    
    for (idx, swap) in swaps.iter().enumerate() {
        println!("\n{} {} Swap #{}", 
                 format!("{}.", idx + 1).bold(),
                 if swap.successful { "✅" } else { "❌" },
                 swap.id.dimmed());
        
        println!("   {} {}:{}", "Pool:".bold(),
                 swap.pool_block_id.cyan(), swap.pool_tx_id.cyan());
        println!("   {} {}:{} → {}:{}", "Trade:".bold(),
                 swap.sold_token_block_id.yellow(), swap.sold_token_tx_id.yellow(),
                 swap.bought_token_block_id.green(), swap.bought_token_tx_id.green());
        println!("   {} {:.4} → {:.4}", "Amount:".bold(),
                 swap.sold_amount, swap.bought_amount);
        
        let price = if swap.sold_amount > 0.0 {
            swap.bought_amount / swap.sold_amount
        } else {
            0.0
        };
        println!("   {} {:.6}", "Price:".bold(), price);
        
        if let Some(ref seller) = swap.seller_address {
            println!("   {} {}", "Trader:".bold(), seller.dimmed());
        }
        
        println!("   {} Block #{}", "Block:".bold(), swap.block_height);
    }
    
    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {} swaps", "Total:".bold(), swaps.len());
    println!();
}

pub fn print_holders_response(response: &serde_json::Value) {
    let alkane = response.get("alkane").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let total = response.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
    let page = response.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let limit = response.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);
    let has_more = response.get("has_more").and_then(|v| v.as_bool()).unwrap_or(false);

    println!("\n{}", format!("👥 Holders for {}", alkane).bold().cyan());
    println!("{}", "═".repeat(80).cyan());

    if let Some(items) = response.get("items").and_then(|v| v.as_array()) {
        if items.is_empty() {
            println!("  {}", "No holders found".yellow());
            println!();
            return;
        }

        for (idx, holder) in items.iter().enumerate() {
            let address = holder.get("address").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let amount = holder.get("amount").and_then(|v| v.as_str()).unwrap_or("0");

            println!("\n  {} {}",
                     format!("{}.", idx + 1).dimmed(),
                     address.bright_white());
            println!("     {} {}", "Balance:".bold(), amount.green());
        }
    } else {
        println!("  {}", "No holder data available".yellow());
    }

    println!("\n{}", "─".repeat(80).cyan());
    println!("{} {} holder(s) | Page {} | {} per page{}",
             "Total:".bold(),
             total,
             page,
             limit,
             if has_more { " | More available" } else { "" });
    println!();
}

pub fn print_holder_count_response(response: &serde_json::Value) {
    let alkane = response.get("alkane").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let count = response.get("count").and_then(|v| v.as_i64()).unwrap_or(0);

    println!("\n{}", format!("👥 Holder Count for {}", alkane).bold().cyan());
    println!("{}", "═".repeat(50).cyan());
    println!("  {} {}", "Count:".bold(), count.to_string().green());
    println!("{}", "─".repeat(50).cyan());
    println!();
}
