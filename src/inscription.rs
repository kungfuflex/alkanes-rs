//! Dogecoin inscription support for ALKANES-RS
//! 
//! This module provides functionality for working with Dogecoin inscriptions
//! in the ALKANES-RS project. It allows for extracting WASM files from
//! Dogecoin inscriptions and using them as smart contracts.

use alkanes_support::doge_inscription::{DogeInscription, ParsedDogeInscription, WASM_CONTENT_TYPE};
use bitcoin::Transaction;
use anyhow::Result;

/// Extract a WASM file from a Dogecoin inscription
/// 
/// This function takes a Bitcoin transaction and attempts to extract a WASM file
/// from a Dogecoin inscription contained within it.
/// 
/// # Arguments
/// 
/// * `tx` - The Bitcoin transaction containing the inscription
/// 
/// # Returns
/// 
/// * `Option<Vec<u8>>` - The WASM file bytes if found, None otherwise
pub fn extract_wasm_from_inscription(tx: &Transaction) -> Option<Vec<u8>> {
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

/// Create a new Dogecoin inscription containing a WASM file
/// 
/// This function creates a new Dogecoin inscription containing a WASM file.
/// 
/// # Arguments
/// 
/// * `wasm_data` - The WASM file bytes
/// 
/// # Returns
/// 
/// * `DogeInscription` - The created inscription
pub fn create_wasm_inscription(wasm_data: Vec<u8>) -> DogeInscription {
    DogeInscription::from_wasm(wasm_data)
}