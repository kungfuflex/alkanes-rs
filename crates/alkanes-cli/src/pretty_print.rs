//! Pretty-printing functions for deezel CLI output
//!
//! This module contains functions for formatting the various data structures
//! returned by the `ord` server into human-readable output.

use alkanes_cli_common::{
    alkanes::protorunes::{ProtoruneOutpointResponse, ProtoruneWalletResponse},
    ord::*,
    traits::UtxoInfo,
};
use alkanes_cli_common::alkanes::AlkaneBalance;
use alkanes_cli_common::traits::TransactionInfo;
use termtree::Tree;
use colored::*;
use bitcoin::OutPoint;

pub fn print_history(history: &[TransactionInfo]) {
    let mut trees = Vec::new();
    for tx in history {
        let mut tx_tree = Tree::new(format!("{} {}", "TXID:".bold(), tx.txid));
        tx_tree.push(Tree::new(format!("{} {}", "Confirmed:".bold(), tx.confirmed)));
        if let Some(h) = tx.block_height {
            tx_tree.push(Tree::new(format!("{} {}", "Block Height:".bold(), h)));
        }
        if let Some(f) = tx.fee {
            tx_tree.push(Tree::new(format!("{} {}", "Fee (sats):".bold(), f)));
        }
        trees.push(tx_tree);
    }
    let root = Tree::new("📜 Transaction History".to_string()).with_leaves(trees);
    println!("{}", root);
}

pub fn print_alkane_balances(balances: &[AlkaneBalance]) {
    let mut trees = Vec::new();
    for balance in balances {
        let mut balance_tree = Tree::new(format!("{} {}:{}", "ID:".bold(), balance.alkane_id.block, balance.alkane_id.tx));
        balance_tree.push(Tree::new(format!("{} {}", "Name:".bold(), balance.name)));
        balance_tree.push(Tree::new(format!("{} {}", "Symbol:".bold(), balance.symbol)));
        balance_tree.push(Tree::new(format!("{} {}", "Balance:".bold(), balance.balance)));
        trees.push(balance_tree);
    }
    let root = Tree::new("🪙 Alkane Balances".to_string()).with_leaves(trees);
    println!("{}", root);
}

pub fn print_utxos(utxos: &[(OutPoint, UtxoInfo)]) {
    let mut trees = Vec::new();
    for (outpoint, utxo_info) in utxos {
        let mut utxo_tree = Tree::new(format!("{} {}:{}", "Outpoint:".bold(), outpoint.txid, outpoint.vout));
        utxo_tree.push(Tree::new(format!("{} {}", "Amount (sats):".bold(), utxo_info.amount)));
        utxo_tree.push(Tree::new(format!("{} {}", "Address:".bold(), utxo_info.address)));
        utxo_tree.push(Tree::new(format!("{} {}", "Confirmations:".bold(), utxo_info.confirmations)));
        if let Some(height) = utxo_info.block_height {
            utxo_tree.push(Tree::new(format!("{} {}", "Block Height:".bold(), height)));
        }
        let mut properties = Vec::new();
        if utxo_info.has_inscriptions {
            properties.push("inscriptions");
        }
        if utxo_info.has_runes {
            properties.push("runes");
        }
        if utxo_info.has_alkanes {
            properties.push("alkanes");
        }
        if utxo_info.is_coinbase {
            properties.push("coinbase");
        }
        if !properties.is_empty() {
            utxo_tree.push(Tree::new(format!("{} {}", "Properties:".bold(), properties.join(", "))));
        }
        if utxo_info.frozen {
            let reason = utxo_info.freeze_reason.as_deref().unwrap_or("No reason provided");
            utxo_tree.push(Tree::new(format!("{} {}", "Status:".bold(), "FROZEN".red())).with_leaves([
                Tree::new(format!("{} {}", "Reason:".bold(), reason))
            ]));
        }
        trees.push(utxo_tree);
    }
    let root = Tree::new("💰 UTXOs".to_string()).with_leaves(trees);
    println!("{}", root);
}

pub fn print_inscription(inscription: &Inscription) {
    println!("{}", serde_json::to_string_pretty(inscription).unwrap());
}

pub fn print_inscriptions(inscriptions: &[Inscription]) {
    println!("{}", serde_json::to_string_pretty(inscriptions).unwrap());
}

pub fn print_address_info(address_info: &AddressInfo) {
    println!("{}", serde_json::to_string_pretty(address_info).unwrap());
}

pub fn print_block_info(block_info: &BlockInfo) {
    println!("{}", serde_json::to_string_pretty(block_info).unwrap());
}

pub fn print_output(output: &Output) {
    println!("{}", serde_json::to_string_pretty(output).unwrap());
}

pub fn print_sat_response(sat_response: &SatResponse) {
    println!("{}", serde_json::to_string_pretty(sat_response).unwrap());
}

pub fn print_children(inscriptions: &[Inscription]) {
    println!("{}", serde_json::to_string_pretty(inscriptions).unwrap());
}

pub fn print_parents(parents: &ParentInscriptions) {
    println!("{}", serde_json::to_string_pretty(parents).unwrap());
}

pub fn print_rune(rune_info: &RuneInfo) {
    println!("{}", serde_json::to_string_pretty(rune_info).unwrap());
}

pub fn print_blocks(blocks: &Blocks) {
    println!("{}", serde_json::to_string_pretty(blocks).unwrap());
}

#[allow(dead_code)]
pub fn print_runes(runes: &Runes) {
    println!("{}", serde_json::to_string_pretty(runes).unwrap());
}

pub fn print_tx_info(tx_info: &TxInfo) {
    println!("{}", serde_json::to_string_pretty(tx_info).unwrap());
}

pub fn print_protorune_outpoint_response(response: &ProtoruneOutpointResponse) {
    let mut root = Tree::new(format!("📦 {}", "Protorune Outpoint Response".bold()));
    let mut outpoint_tree = Tree::new(format!("{} {}", "Outpoint:".bold(), response.outpoint));
    outpoint_tree.push(Tree::new(format!("{} {} sats", "Value:".bold(), response.output.value)));
    outpoint_tree.push(Tree::new(format!("{} {}", "Script Pubkey:".bold(), response.output.script_pubkey)));
    
    let mut balance_sheet_tree = Tree::new("📜 Balance Sheet".to_string());
    for (rune_id, balance) in &response.balance_sheet.cached.balances {
        let mut rune_tree = Tree::new(format!("{} {}:{}", "Rune ID:".bold(), rune_id.block, rune_id.tx));
        rune_tree.push(Tree::new(format!("{} {balance}", "Balance:".bold())));
        balance_sheet_tree.push(rune_tree);
    }
    outpoint_tree.push(balance_sheet_tree);
    root.push(outpoint_tree);
    println!("{}", root);
}

pub fn print_protorune_wallet_response(response: &ProtoruneWalletResponse) {
    println!("💰 Protorune Wallet Balances");
    println!("===========================");
    for balance in &response.balances {
        print_protorune_outpoint_response(balance);
        println!();
    }
}

pub fn print_inspection_result(result: &alkanes_cli_common::alkanes::types::AlkanesInspectResult) {
    let mut root = Tree::new(format!("🔍 Inspection Result for Alkane: {}:{}", result.alkane_id.block, result.alkane_id.tx));
    root.push(Tree::new(format!("📏 Bytecode Length: {} bytes", result.bytecode_length)));

    if let Some(codehash) = &result.codehash {
        root.push(Tree::new(format!("🔑 Code Hash: {codehash}")));
    }

    if let Some(disassembly) = &result.disassembly {
        root.push(Tree::new(format!("\n disassembled bytecode:\n{disassembly}")));
    }

    if let Some(metadata) = &result.metadata {
        let metadata_str = serde_json::to_string_pretty(metadata).unwrap_or_else(|e| e.to_string());
        root.push(Tree::new("📝 Metadata:".to_string()).with_leaves([metadata_str]));
    }

    if let Some(metadata_error) = &result.metadata_error {
        root.push(Tree::new(format!("⚠️ Metadata Error: {metadata_error}")));
    }

    if let Some(fuzzing_results) = &result.fuzzing_results {
        let mut fuzz_tree = Tree::new("🔬 Fuzzing Results:".to_string());
        for result in &fuzzing_results.opcode_results {
            let status = if result.success { "Success".green() } else { "Failure".red() };
            fuzz_tree.push(Tree::new(format!("  - Opcode 0x{:02X}: {}", result.opcode, status)));
        }
        root.push(fuzz_tree);
    }
    println!("{}", root);
}

pub fn print_esplora_transactions(txs: &[alkanes_cli_common::esplora::EsploraTransaction]) {
    if txs.is_empty() {
        println!("📭 No transactions found");
        return;
    }

    let mut trees = Vec::new();
    for tx in txs {
        let status_icon = if tx.status.as_ref().map_or(false, |s| s.confirmed) {
            "✅"
        } else {
            "⏳"
        };
        
        let mut tx_tree = Tree::new(format!("{} {} {}", status_icon, "TXID:".bold(), tx.txid.bright_cyan()));
        
        // Status info
        if let Some(status) = &tx.status {
            let mut status_tree = Tree::new(format!("{}", "Status:".bold()));
            if status.confirmed {
                status_tree.push(Tree::new(format!("✅ {}", "Confirmed".green())));
                if let Some(height) = status.block_height {
                    status_tree.push(Tree::new(format!("{} {}", "Block Height:".bold(), height)));
                }
                if let Some(hash) = &status.block_hash {
                    status_tree.push(Tree::new(format!("{} {}...", "Block Hash:".bold(), &hash[..16])));
                }
                if let Some(time) = status.block_time {
                    status_tree.push(Tree::new(format!("{} {}", "Block Time:".bold(), time)));
                }
            } else {
                status_tree.push(Tree::new(format!("⏳ {}", "Mempool".yellow())));
            }
            tx_tree.push(status_tree);
        }
        
        // Basic tx info
        tx_tree.push(Tree::new(format!("{} {} sats", "Fee:".bold(), tx.fee.to_string().bright_yellow())));
        tx_tree.push(Tree::new(format!("{} {}", "Size:".bold(), tx.size)));
        tx_tree.push(Tree::new(format!("{} {}", "Weight:".bold(), tx.weight)));
        tx_tree.push(Tree::new(format!("{} {}", "Version:".bold(), tx.version)));
        tx_tree.push(Tree::new(format!("{} {}", "Locktime:".bold(), tx.locktime)));
        
        // Inputs
        let mut inputs_tree = Tree::new(format!("{} {} inputs", "Inputs:".bold(), tx.vin.len().to_string().bright_blue()));
        for (i, vin) in tx.vin.iter().enumerate() {
            if vin.is_coinbase {
                inputs_tree.push(Tree::new(format!("{} {} {}", i.to_string().dimmed(), "⛏️".bold(), "Coinbase".bright_green())));
            } else {
                let mut input_tree = Tree::new(format!("{} {}:{}", i.to_string().dimmed(), &vin.txid[..16], vin.vout));
                if let Some(prevout) = &vin.prevout {
                    input_tree.push(Tree::new(format!("{} {} sats", "Value:".bold(), prevout.value.to_string().bright_yellow())));
                    if let Some(addr) = &prevout.scriptpubkey_address {
                        input_tree.push(Tree::new(format!("{} {}", "Address:".bold(), addr)));
                    }
                    input_tree.push(Tree::new(format!("{} {}", "Type:".bold(), prevout.scriptpubkey_type)));
                }
                if let Some(witness) = &vin.witness {
                    if !witness.is_empty() {
                        let witness_str = if witness.len() > 1 {
                            format!("{} items", witness.len())
                        } else {
                            "1 item".to_string()
                        };
                        input_tree.push(Tree::new(format!("{} {}", "Witness:".bold(), witness_str.dimmed())));
                    }
                }
                inputs_tree.push(input_tree);
            }
        }
        tx_tree.push(inputs_tree);
        
        // Outputs
        let mut outputs_tree = Tree::new(format!("{} {} outputs", "Outputs:".bold(), tx.vout.len().to_string().bright_blue()));
        for (i, vout) in tx.vout.iter().enumerate() {
            let output_icon = match vout.scriptpubkey_type.as_str() {
                "op_return" => "📝",
                "v1_p2tr" => "🔑",
                "v0_p2wpkh" | "v0_p2wsh" => "⚡",
                _ => "📤",
            };
            
            let mut output_tree = Tree::new(format!("{} {} {} sats", 
                i.to_string().dimmed(), 
                output_icon, 
                vout.value.to_string().bright_yellow()
            ));
            
            if let Some(addr) = &vout.scriptpubkey_address {
                output_tree.push(Tree::new(format!("{} {}", "Address:".bold(), addr)));
            }
            output_tree.push(Tree::new(format!("{} {}", "Type:".bold(), vout.scriptpubkey_type)));
            
            if vout.scriptpubkey_type == "op_return" {
                output_tree.push(Tree::new(format!("{} {}", "Data:".bold(), &vout.scriptpubkey_asm.dimmed())));
            }
            
            outputs_tree.push(output_tree);
        }
        tx_tree.push(outputs_tree);
        
        trees.push(tx_tree);
    }
    
    let root = Tree::new(format!("📄 {} transaction{}", txs.len(), if txs.len() == 1 { "" } else { "s" }))
        .with_leaves(trees);
    println!("{}", root);
}
