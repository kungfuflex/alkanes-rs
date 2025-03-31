//! An example of a Metashrew indexer program using Protocol Buffers.

use anyhow::Result;
use metashrew_core::{declare_indexer, indexer::{Indexer, KeyValueStore}};
use protobuf::Message;

// Define Protocol Buffer messages
#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceRequest {
    #[prost(string, tag = "1")]
    pub address: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceResponse {
    #[prost(uint64, tag = "1")]
    pub balance: u64,
    #[prost(uint32, tag = "2")]
    pub last_updated: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetTotalSupplyRequest {
    // Empty request
}

#[derive(Clone, PartialEq, Message)]
pub struct GetTotalSupplyResponse {
    #[prost(uint64, tag = "1")]
    pub total_supply: u64,
}

/// A simple token indexer
struct TokenIndexer {
    store: KeyValueStore,
}

impl Default for TokenIndexer {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
        }
    }
}

impl Indexer for TokenIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        // In a real implementation, we would parse the block and extract token transfers
        // For this example, we'll just simulate some token activity
        
        // Update the balance for an example address
        let address = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        let balance_key = format!("balance:{}", address).into_bytes();
        
        // Get the current balance or default to 0
        let current_balance = if let Some(balance_bytes) = self.store.get(&balance_key) {
            u64::from_le_bytes([
                balance_bytes[0], balance_bytes[1], balance_bytes[2], balance_bytes[3],
                balance_bytes[4], balance_bytes[5], balance_bytes[6], balance_bytes[7],
            ])
        } else {
            0
        };
        
        // Increase the balance by the block height (just for demonstration)
        let new_balance = current_balance + height as u64;
        
        // Store the updated balance
        self.store.set(balance_key, new_balance.to_le_bytes().to_vec());
        
        // Store the last updated height for this address
        let last_updated_key = format!("last_updated:{}", address).into_bytes();
        self.store.set(last_updated_key, height.to_le_bytes().to_vec());
        
        // Update the total supply
        let total_supply_key = b"total_supply".to_vec();
        let current_supply = if let Some(supply_bytes) = self.store.get(&total_supply_key) {
            u64::from_le_bytes([
                supply_bytes[0], supply_bytes[1], supply_bytes[2], supply_bytes[3],
                supply_bytes[4], supply_bytes[5], supply_bytes[6], supply_bytes[7],
            ])
        } else {
            0
        };
        
        let new_supply = current_supply + height as u64;
        self.store.set(total_supply_key, new_supply.to_le_bytes().to_vec());
        
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

impl TokenIndexer {
    /// Get the balance for an address
    fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        let address = request.address;
        let balance_key = format!("balance:{}", address).into_bytes();
        let last_updated_key = format!("last_updated:{}", address).into_bytes();
        
        // Get the balance
        let balance = if let Some(balance_bytes) = self.store.get(&balance_key) {
            u64::from_le_bytes([
                balance_bytes[0], balance_bytes[1], balance_bytes[2], balance_bytes[3],
                balance_bytes[4], balance_bytes[5], balance_bytes[6], balance_bytes[7],
            ])
        } else {
            0
        };
        
        // Get the last updated height
        let last_updated = if let Some(height_bytes) = self.store.get(&last_updated_key) {
            u32::from_le_bytes([
                height_bytes[0], height_bytes[1], height_bytes[2], height_bytes[3],
            ])
        } else {
            0
        };
        
        let mut response = GetBalanceResponse::new();
        response.balance = balance;
        response.last_updated = last_updated;
        
        Ok(response)
    }
    
    /// Get the total token supply
    fn get_total_supply(&self, _request: GetTotalSupplyRequest) -> Result<GetTotalSupplyResponse> {
        let total_supply_key = b"total_supply".to_vec();
        
        // Get the total supply
        let total_supply = if let Some(supply_bytes) = self.store.get(&total_supply_key) {
            u64::from_le_bytes([
                supply_bytes[0], supply_bytes[1], supply_bytes[2], supply_bytes[3],
                supply_bytes[4], supply_bytes[5], supply_bytes[6], supply_bytes[7],
            ])
        } else {
            0
        };
        
        let mut response = GetTotalSupplyResponse::new();
        response.total_supply = total_supply;
        
        Ok(response)
    }
}

// Define the Metashrew indexer program with Protocol Buffer messages
declare_indexer! {
    struct TokenIndexerProgram {
        indexer: TokenIndexer,
        views: {
            "get_balance" => {
                fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
                    self.get_balance(request)
                }
            },
            "get_total_supply" => {
                fn get_total_supply(&self, request: GetTotalSupplyRequest) -> Result<GetTotalSupplyResponse> {
                    self.get_total_supply(request)
                }
            }
        }
    }
}

// This is just for the example to compile
fn main() {
    println!("This is an example of a Metashrew indexer program using Protocol Buffers.");
    println!("It should be compiled to WebAssembly and loaded by Metashrew.");
}
