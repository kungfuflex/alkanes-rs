mod protorune;

use anyhow::{Context, Result};
use bitcoin::Transaction;
use clap::Parser;
use log::info;
use std::io::Cursor;

/// CLI tool to enrich Bitcoin transactions with protorune data
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Raw transaction hex
    #[clap(short, long)]
    tx_hex: String,

    /// Block height where the transaction was confirmed
    #[clap(short, long)]
    block_height: u32,

    /// Verbose output
    #[clap(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    // Parse command line arguments
    let args = Args::parse();

    if args.verbose {
        println!("Transaction hex: {}", args.tx_hex);
        println!("Block height: {}", args.block_height);
    }

    // Decode the raw transaction
    let tx_bytes = hex::decode(&args.tx_hex)
        .context("Failed to decode transaction hex")?;
    
    let tx: Transaction = metashrew_support::utils::consensus_decode(
        &mut Cursor::new(tx_bytes)
    ).context("Failed to parse transaction")?;

    info!("Transaction ID: {}", tx.compute_txid());
    
    // Process transaction inputs
    process_tx_inputs(&tx, args.block_height)?;
    
    // Process transaction outputs
    process_tx_outputs(&tx, args.block_height)?;
    
    // Decode OP_RETURN output if present
    decode_op_return(&tx)?;

    Ok(())
}

fn process_tx_inputs(tx: &Transaction, block_height: u32) -> Result<()> {
    println!("\n=== Transaction Inputs ===");
    
    for (i, input) in tx.input.iter().enumerate() {
        let prev_outpoint = &input.previous_output;
        
        println!("Input #{}: {}:{}", i, prev_outpoint.txid, prev_outpoint.vout);
        println!("  Would query protorunes at block height: {}", block_height.saturating_sub(1));
    }
    
    Ok(())
}

fn process_tx_outputs(tx: &Transaction, block_height: u32) -> Result<()> {
    println!("\n=== Transaction Outputs ===");
    
    for (i, output) in tx.output.iter().enumerate() {
        // Try to extract address from script
        let address = match bitcoin::Address::from_script(&output.script_pubkey, bitcoin::Network::Bitcoin) {
            Ok(addr) => addr.to_string(),
            Err(_) => "Unknown".to_string(),
        };
        
        println!("Output #{}: {} satoshis to {}", i, output.value, address);
        
        // Skip OP_RETURN outputs for protorunes query
        if !output.script_pubkey.is_op_return() {
            println!("  Would query protorunes at block height: {}", block_height);
        }
    }
    
    Ok(())
}

fn decode_op_return(tx: &Transaction) -> Result<()> {
    println!("\n=== OP_RETURN Decoding ===");
    
    // Find and decode OP_RETURN outputs
    let protostones = protorune::decode_op_return_outputs(tx)?;
    
    if protostones.is_empty() {
        println!("No OP_RETURN outputs found in transaction");
        return Ok(());
    }
    
    for (i, protostone_vec_opt) in protostones.iter().enumerate() {
        println!("OP_RETURN #{}", i);
        
        match protostone_vec_opt {
            Some(protostone_vec) => {
                if protostone_vec.is_empty() {
                    println!("Decoded as Runestone but no Protostones found");
                } else {
                    for (j, protostone) in protostone_vec.iter().enumerate() {
                        println!("Protostone #{}: {:?}", j, protostone);
                    }
                }
            },
            None => {
                // Find the corresponding output to get raw data
                for output in &tx.output {
                    if output.script_pubkey.is_op_return() {
                        println!("Could not decode as Protostone");
                        println!("Raw data: {}", hex::encode(output.script_pubkey.as_bytes()));
                        break;
                    }
                }
            }
        }
    }
    
    Ok(())
}
