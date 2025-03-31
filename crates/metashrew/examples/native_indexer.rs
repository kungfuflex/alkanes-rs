//! An example of a native Metashrew indexer program.

#[cfg(feature = "native")]
use anyhow::Result;
#[cfg(feature = "native")]
use metashrew_core::{
    indexer::{Indexer, KeyValueStore, NativeIndexer, ProtoViewFunctionWrapper, ViewFunctionWrapper},
    native_binary,
    view::ProtoViewFunction,
};
#[cfg(feature = "native")]
use protobuf::Message;
#[cfg(feature = "native")]
use std::collections::HashMap;

#[cfg(feature = "native")]
#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceRequest {
    #[prost(string, tag = "1")]
    pub address: String,
}

#[cfg(feature = "native")]
#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceResponse {
    #[prost(uint64, tag = "1")]
    pub balance: u64,
    #[prost(uint32, tag = "2")]
    pub last_updated: u32,
}

#[cfg(feature = "native")]
#[derive(Clone, PartialEq, Message)]
pub struct GetTotalSupplyRequest {
    // Empty request
}

#[cfg(feature = "native")]
#[derive(Clone, PartialEq, Message)]
pub struct GetTotalSupplyResponse {
    #[prost(uint64, tag = "1")]
    pub total_supply: u64,
}

#[cfg(feature = "native")]
/// A simple token indexer
#[derive(Clone)]
struct TokenIndexer {
    store: KeyValueStore,
    height: u32,
}

#[cfg(feature = "native")]
impl Default for TokenIndexer {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
            height: 0,
        }
    }
}

#[cfg(feature = "native")]
impl Indexer for TokenIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        // In a real implementation, we would parse the block and extract token transfers
        // For this example, we'll just simulate some token activity
        self.height = height;
        
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

#[cfg(feature = "native")]
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

#[cfg(feature = "native")]
impl ProtoViewFunction<GetBalanceRequest, GetBalanceResponse> for TokenIndexer {
    fn execute_proto(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        self.get_balance(request)
    }
}

#[cfg(feature = "native")]
impl ProtoViewFunction<GetTotalSupplyRequest, GetTotalSupplyResponse> for TokenIndexer {
    fn execute_proto(&self, request: GetTotalSupplyRequest) -> Result<GetTotalSupplyResponse> {
        self.get_total_supply(request)
    }
}

#[cfg(feature = "native")]
impl NativeIndexer for TokenIndexer {
    fn view_functions(&self) -> HashMap<String, Box<dyn ViewFunctionWrapper>> {
        let mut map = HashMap::new();
        
        // Add the get_balance view function
        map.insert(
            "get_balance".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, GetBalanceRequest, GetBalanceResponse>::new(
                self.clone(),
            )),
        );
        
        // Add the get_total_supply view function
        map.insert(
            "get_total_supply".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, GetTotalSupplyRequest, GetTotalSupplyResponse>::new(
                self.clone(),
            )),
        );
        
        map
    }
}

#[cfg(feature = "native")]
native_binary! {
    indexer: TokenIndexer,
    name: "token-indexer",
    version: "0.1.0",
    about: "A simple token indexer",
}

#[cfg(not(feature = "native"))]
fn main() {
    println!("This example requires the 'native' feature to be enabled.");
    println!("Run with: cargo run --example native_indexer --features native");
}
