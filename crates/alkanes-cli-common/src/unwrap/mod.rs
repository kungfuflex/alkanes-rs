//! Generic unwrap trait for different metaprotocol implementations
//!
//! This module provides a trait-based abstraction for fetching pending unwraps
//! from different metaprotocol implementations:
//! - Alkanes: Uses metashrew_view RPC to query the alkanes indexer
//! - BRC20-Prog: Uses eth_call to query FrBTC.sol contract

use crate::{AlkanesError, Result};
use crate::alkanes::PendingUnwrap;
use crate::traits::DeezelProvider;
use async_trait::async_trait;

pub mod alkanes;
pub mod brc20_prog;

// Re-export implementations and helper functions for external use
pub use brc20_prog::{Brc20ProgUnwrap, script_pubkey_to_address, find_oldest_546_sat_utxo, payment_to_pending_unwrap, parse_abi_encoded_payments};
pub use alkanes::AlkanesUnwrap;

/// Trait for fetching pending unwraps from a metaprotocol implementation
#[async_trait(?Send)]
pub trait MetaprotocolUnwrap {
    /// Get the list of pending unwraps (unfiltered by wallet)
    ///
    /// # Arguments
    /// * `provider` - The RPC provider to use for queries
    /// * `confirmations_required` - Number of confirmations to wait before considering unwraps
    ///
    /// # Returns
    /// A vector of unfiltered pending unwraps. Caller should filter by wallet UTXOs.
    async fn get_pending_unwraps(
        &self,
        provider: &dyn DeezelProvider,
        confirmations_required: u64,
    ) -> Result<Vec<PendingUnwrap>>;
    
    /// Get the total supply of frBTC for this protocol
    ///
    /// # Arguments
    /// * `provider` - The RPC provider to use for queries
    ///
    /// # Returns
    /// The total supply in satoshis
    async fn get_total_supply(
        &self,
        provider: &dyn DeezelProvider,
    ) -> Result<u64>;
    
    /// Get the protocol name for logging/debugging
    fn protocol_name(&self) -> &'static str;
}

/// Protocol type for unwrap operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwrapProtocol {
    /// Alkanes protocol using metashrew indexer
    Alkanes,
    /// BRC20-Prog protocol using FrBTC.sol contract
    Brc20Prog,
}

impl UnwrapProtocol {
    /// Create an unwrap implementation for this protocol
    pub fn create_unwrap_impl(&self) -> Box<dyn MetaprotocolUnwrap> {
        match self {
            UnwrapProtocol::Alkanes => Box::new(alkanes::AlkanesUnwrap::new()),
            UnwrapProtocol::Brc20Prog => Box::new(brc20_prog::Brc20ProgUnwrap::new()),
        }
    }
}
