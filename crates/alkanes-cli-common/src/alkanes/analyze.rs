//! # Runestone Analysis and Decoding
//!
//! This module provides functionality for analyzing and decoding Runestones
//! from Bitcoin transactions, with a special focus on decoding the `protocol`
//! field which contains cellpack data for Alkanes.

use anyhow::Result;
use bitcoin::Transaction;
use serde_json::{Value};


use crate::runestone_enhanced;

/// Analyzes a transaction to find and decode a Runestone.
///
/// This function will:
/// 1. Decipher the Runestone from the transaction using the `ord` crate.
/// 2. If a Runestone is found, it will decode the `protocol` field into a Cellpack.
/// 3. It returns a `serde_json::Value` containing the decoded information.
pub fn analyze_runestone(tx: &Transaction) -> Result<Value> {
    runestone_enhanced::format_runestone_with_decoded_messages(tx)
}