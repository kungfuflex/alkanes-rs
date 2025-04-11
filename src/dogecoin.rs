use alkanes_support::doge_inscription::{DogeInscription, ParsedDogeInscription, BIN_PROTOCOL_ID, WASM_CONTENT_TYPE};
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use std::sync::Arc;

#[cfg(feature = "dogecoin")]
pub fn find_wasm_in_dogecoin_inscription(tx: &Transaction) -> Option<Vec<u8>> {
    // Extract inscriptions from the transaction
    let inscriptions = DogeInscription::from_transactions(vec![tx.clone()]);
    
    // Check if we have a complete inscription
    if let ParsedDogeInscription::Complete(inscription) = inscriptions {
        // Check if it's a WASM file
        if inscription.content_type() == Some(WASM_CONTENT_TYPE) {
            return inscription.extract_wasm();
        }
    }
    
    None
}

#[cfg(feature = "dogecoin")]
pub fn load_wasm_from_dogecoin_inscription(tx: &Transaction, _target: &AlkaneId) -> Result<Arc<Vec<u8>>> {
    // Try to find WASM in the transaction
    let wasm_payload = find_wasm_in_dogecoin_inscription(tx)
        .ok_or_else(|| anyhow!("No WASM found in Dogecoin inscription"))?;
    
    // We'll assume the WASM is already in the correct format
    Ok(Arc::new(wasm_payload))
}

#[cfg(feature = "dogecoin")]
pub fn create_dogecoin_wasm_inscription(wasm_data: Vec<u8>) -> DogeInscription {
    // Create a new inscription with WASM content type
    DogeInscription::from_wasm(wasm_data)
}