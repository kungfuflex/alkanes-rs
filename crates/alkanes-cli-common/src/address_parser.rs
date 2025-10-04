//! # Address Parser
//!
//! This module provides functionality for parsing address specifications.
//! It can handle various formats, including:
//! - Single addresses (e.g., `bc1q...`)
//! - Address types with index ranges (e.g., `p2tr:0-100`)
//! - Address types with comma-separated indices (e.g., `p2wpkh:0,1,5`)
//! - Address types with a single index (e.g., `p2sh:10`)

use crate::{Result, DeezelError};
use crate::traits::AddressResolver;
use bitcoin::{Address, address::NetworkChecked};
use alloc::{string::{String, ToString}, vec::Vec, str::FromStr};

#[derive(Clone)]
pub struct AddressParser<R: AddressResolver> {
    address_resolver: R,
}

impl<R: AddressResolver> AddressParser<R> {
    pub fn new(address_resolver: R) -> Self {
        Self { address_resolver }
    }

    pub async fn parse(&self, spec: &str) -> Result<Vec<String>> {
        // Check if it's a plain address
        if let Ok(address) = Address::from_str(spec) {
            let checked_address: Address<NetworkChecked> = address.require_network(bitcoin::Network::Bitcoin).map_err(|_| DeezelError::InvalidParameters("Address has an invalid network".to_string()))?;
            return Ok(vec![checked_address.to_string()]);
        }

        // Try to parse as a range or list
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() != 2 {
            return Err(DeezelError::Parse(format!("Invalid address specifier: {}", spec)));
        }

        let address_type = parts[0];
        let indices_part = parts[1];

        let mut indices = Vec::new();
        if indices_part.contains('-') {
            // Range
            let range_parts: Vec<&str> = indices_part.split('-').collect();
            if range_parts.len() != 2 {
                return Err(DeezelError::Parse(format!("Invalid range: {}", indices_part)));
            }
            let start = range_parts[0].parse::<u32>()?;
            let end = range_parts[1].parse::<u32>()?;
            for i in start..=end {
                indices.push(i);
            }
        } else if indices_part.contains(',') {
            // Comma-separated list
            for s in indices_part.split(',') {
                indices.push(s.parse::<u32>()?);
            }
        } else {
            // Single index
            indices.push(indices_part.parse::<u32>()?);
        }

        let mut addresses = Vec::new();
        for index in indices {
            let address = self.address_resolver.get_address(address_type, index).await?;
            addresses.push(address);
        }

        Ok(addresses)
    }
}