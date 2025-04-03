use anyhow::Result;
use bitcoin::Transaction;
use ordinals::runestone::Runestone;
use ordinals::Artifact;
use protorune_support::protostone::Protostone;

/// Decode OP_RETURN outputs in a transaction as Runestone/Protostone
pub fn decode_op_return_outputs(tx: &Transaction) -> Result<Vec<Option<Vec<Protostone>>>> {
    let mut results = Vec::new();
    
    // Try to decode the transaction as a runestone
    if let Some(artifact) = Runestone::decipher(tx) {
        if let Artifact::Runestone(runestone) = artifact {
            // Convert to Protostone
            match Protostone::from_runestone(&runestone) {
                Ok(protostones) => {
                    results.push(Some(protostones));
                },
                Err(e) => {
                    log::warn!("Failed to convert to Protostone: {}", e);
                    results.push(None);
                }
            }
        } else {
            log::info!("Decoded as non-Runestone artifact");
            results.push(None);
        }
    } else {
        // If no runestone was found, check for OP_RETURN outputs
        for output in &tx.output {
            if output.script_pubkey.is_op_return() {
                log::info!("Found OP_RETURN output but not a valid Runestone");
                results.push(None);
            }
        }
    }
    
    Ok(results)
}

/// Format balance sheet for display
pub fn format_balance_sheet(balance_sheet: &serde_json::Value) -> String {
    match serde_json::to_string_pretty(balance_sheet) {
        Ok(formatted) => formatted,
        Err(_) => "Error formatting balance sheet".to_string(),
    }
}