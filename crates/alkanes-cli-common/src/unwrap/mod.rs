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
