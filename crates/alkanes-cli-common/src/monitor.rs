//! Blockchain monitoring functionality
//!
//! This module provides comprehensive blockchain monitoring including:
//! - Block monitoring and event detection
//! - Transaction monitoring
//! - Address monitoring
//! - Runestone and alkanes event detection

use crate::{Result, ToString, format};
use crate::traits::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[cfg(not(target_arch = "wasm32"))]
use std::{collections::HashMap, vec::Vec, string::String};
#[cfg(target_arch = "wasm32")]
use alloc::{collections::BTreeMap as HashMap, vec::Vec, string::String};

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, SystemTime};
#[cfg(target_arch = "wasm32")]
use core::time::Duration;

// WASM-compatible time type
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
pub struct SystemTime(f64);

#[cfg(target_arch = "wasm32")]
impl SystemTime {
    pub fn now() -> Self {
        SystemTime(js_sys::Date::now())
    }
    
    pub fn duration_since(&self, earlier: SystemTime) -> core::result::Result<Duration, ()> {
        let diff = self.0 - earlier.0;
        if diff >= 0.0 {
            Ok(Duration::from_millis(diff as u64))
        } else {
            Err(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Serialize for SystemTime {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

#[cfg(target_arch = "wasm32")]
impl<'de> Deserialize<'de> for SystemTime {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let timestamp = f64::deserialize(deserializer)?;
        Ok(SystemTime(timestamp))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Block monitor that works with any provider
pub struct BlockMonitor<P: DeezelProvider> {
    provider: P,
    config: MonitorConfig,
    state: MonitorStats,
}

#[cfg(not(target_arch = "wasm32"))]
impl<P: DeezelProvider> BlockMonitor<P> {
    /// Create a new block monitor
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            config: MonitorConfig::default(),
            state: MonitorStats::default(),
        }
    }
    
    /// Create block monitor with custom configuration
    pub fn with_config(provider: P, config: MonitorConfig) -> Self {
        Self {
            provider,
            config,
            state: MonitorStats::default(),
        }
    }
    
    /// Start monitoring blocks
    pub async fn start_monitoring(&mut self, start_height: Option<u64>) -> Result<()> {
        let start_height = start_height.unwrap_or({
            // Get current height as default
            0 // This would be replaced with actual current height
        });
        
        self.state.current_height = start_height;
        self.state.is_running = true;
        self.state.start_time = Some(SystemTime::now());
        
        self.provider.info(&format!("Starting block monitoring from height {start_height}"));
        
        while self.state.is_running {
            match self.check_new_blocks().await {
                Ok(new_blocks) => {
                    for block_height in new_blocks {
                        if let Err(e) = self.process_block(block_height).await {
                            self.provider.error(&format!("Error processing block {block_height}: {e}"));
                        }
                    }
                }
                Err(e) => {
                    self.provider.error(&format!("Error checking for new blocks: {e}"));
                }
            }
            
            // Sleep for the configured interval
            let sleep_fut = self.provider.sleep_ms(self.config.poll_interval_ms);
            Box::pin(sleep_fut).await;
        }
        
        Ok(())
    }
    
    /// Stop monitoring
    pub fn stop_monitoring(&mut self) {
        self.state.is_running = false;
        self.provider.info("Stopping block monitoring");
    }
    
    /// Check for new blocks
    async fn check_new_blocks(&mut self) -> Result<Vec<u64>> {
        let current_height = self.get_current_blockchain_height().await?;
        let mut new_blocks = Vec::new();
        
        if current_height > self.state.current_height {
            for height in (self.state.current_height + 1)..=current_height {
                new_blocks.push(height);
            }
            self.state.current_height = current_height;
        }
        
        Ok(new_blocks)
    }
    
    /// Get current blockchain height
    async fn get_current_blockchain_height(&self) -> Result<u64> {
        // Try Metashrew first, fall back to Bitcoin RPC
        match self.provider.get_metashrew_height().await {
            Ok(height) => Ok(height),
            Err(_) => <P as BitcoinRpcProvider>::get_block_count(&self.provider).await,
        }
    }
    
    /// Process a single block
    async fn process_block(&mut self, height: u64) -> Result<()> {
        self.provider.debug(&format!("Processing block {height}"));
        
        let events = self.provider.get_block_events(height).await?;
        let events_count = events.len();
        
        for event in events {
            self.process_block_event(&self.convert_block_event(&event)).await?;
        }
        
        self.state.blocks_processed += 1;
        
        // Emit block processed event
        if let Some(callback) = &self.config.block_callback {
            callback(BlockEvent {
                event_type: "block_processed".to_string(),
                block_height: height,
                txid: String::new(),
                data: serde_json::json!({
                    "height": height,
                    "events_count": events_count
                }),
            });
        }
        
        Ok(())
    }
    
    /// Convert traits::BlockEvent to monitor::BlockEvent
    fn convert_block_event(&self, event: &crate::traits::BlockEvent) -> BlockEvent {
        BlockEvent {
            event_type: event.event_type.clone(),
            block_height: event.block_height,
            txid: event.txid.clone(),
            data: event.data.clone(),
        }
    }
    
    /// Process a block event
    async fn process_block_event(&mut self, event: &BlockEvent) -> Result<()> {
        match event.event_type.as_str() {
            "transaction" => self.process_transaction_event(event).await?,
            "runestone" => self.process_runestone_event(event).await?,
            "alkanes" => self.process_alkanes_event(event).await?,
            "protorunes" => self.process_protorunes_event(event).await?,
            _ => {
                self.provider.debug(&format!("Unknown event type: {}", event.event_type));
            }
        }
        
        self.state.events_processed += 1;
        Ok(())
    }
    
    /// Process transaction event
    async fn process_transaction_event(&mut self, event: &BlockEvent) -> Result<()> {
        if let Some(callback) = &self.config.transaction_callback {
            callback(event.clone());
        }
        
        // Check if transaction involves monitored addresses
        if let Some(addresses) = &self.config.monitored_addresses {
            if self.transaction_involves_addresses(&event.txid, addresses).await? {
                self.provider.info(&format!("Transaction {} involves monitored address", event.txid));
                
                if let Some(callback) = &self.config.address_callback {
                    callback(event.clone());
                }
            }
        }
        
        Ok(())
    }
    
    /// Process runestone event
    async fn process_runestone_event(&mut self, event: &BlockEvent) -> Result<()> {
        self.provider.info(&format!("Runestone event in block {}: {}", event.block_height, event.txid));
        
        if let Some(callback) = &self.config.runestone_callback {
            callback(event.clone());
        }
        
        Ok(())
    }
    
    /// Process alkanes event
    async fn process_alkanes_event(&mut self, event: &BlockEvent) -> Result<()> {
        self.provider.info(&format!("Alkanes event in block {}: {}", event.block_height, event.txid));
        
        if let Some(callback) = &self.config.alkanes_callback {
            callback(event.clone());
        }
        
        Ok(())
    }
    
    /// Process protorunes event
    async fn process_protorunes_event(&mut self, event: &BlockEvent) -> Result<()> {
        self.provider.info(&format!("Protorunes event in block {}: {}", event.block_height, event.txid));
        
        if let Some(callback) = &self.config.protorunes_callback {
            callback(event.clone());
        }
        
        Ok(())
    }
    
    /// Check if transaction involves monitored addresses
    async fn transaction_involves_addresses(&self, txid: &str, addresses: &[String]) -> Result<bool> {
        // Get transaction details
        match self.provider.get_transaction_hex(txid).await {
            Ok(tx_hex) => {
                // Parse transaction from hex
                let tx_bytes = hex::decode(&tx_hex)
                    .map_err(|e| crate::DeezelError::Parse(format!("Invalid hex: {e}")))?;
                let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&tx_bytes)
                    .map_err(|e| crate::DeezelError::Parse(format!("Invalid transaction: {e}")))?;
                
                // Check inputs and outputs for monitored addresses
                for input in &tx.input {
                    // For inputs, we'd need to look up the previous output to get the address
                    // This is a simplified check - in practice you'd need to resolve the input addresses
                    let prev_txid = input.previous_output.txid.to_string();
                    if addresses.iter().any(|addr| prev_txid.contains(addr)) {
                        return Ok(true);
                    }
                }
                
                for output in &tx.output {
                    // Extract address from script_pubkey
                    if let Ok(address) = bitcoin::Address::from_script(&output.script_pubkey, bitcoin::Network::Bitcoin) {
                        let addr_str = address.to_string();
                        if addresses.contains(&addr_str) {
                            return Ok(true);
                        }
                    }
                }
                
                Ok(false)
            }
            Err(_) => {
                // If we can't get the transaction, assume it doesn't involve monitored addresses
                Ok(false)
            }
        }
    }
    
    /// Get monitoring statistics
    pub fn get_stats(&self) -> MonitorStats {
        let uptime = self.state.start_time
            .map(|start| SystemTime::now().duration_since(start).unwrap_or(Duration::ZERO))
            .unwrap_or(Duration::ZERO);
        
        MonitorStats {
            is_running: self.state.is_running,
            current_height: self.state.current_height,
            blocks_processed: self.state.blocks_processed,
            events_processed: self.state.events_processed,
            uptime_seconds: uptime.as_secs(),
            start_time: self.state.start_time,
        }
    }
    
    /// Add address to monitoring list
    pub fn add_monitored_address(&mut self, address: String) {
        if self.config.monitored_addresses.is_none() {
            self.config.monitored_addresses = Some(Vec::new());
        }
        
        if let Some(addresses) = &mut self.config.monitored_addresses {
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
    }
    
    /// Remove address from monitoring list
    pub fn remove_monitored_address(&mut self, address: &str) {
        if let Some(addresses) = &mut self.config.monitored_addresses {
            addresses.retain(|a| a != address);
        }
    }
    
    /// Monitor blocks (delegate to start_monitoring)
    pub async fn monitor_blocks(&mut self, start: Option<u64>) -> Result<()> {
        self.start_monitoring(start).await
    }
    
    /// Get block events for a specific height
    pub async fn get_block_events(&self, height: u64) -> Result<Vec<BlockEvent>> {
        let events = self.provider.get_block_events(height).await?;
        Ok(events.into_iter().map(|e| self.convert_block_event(&e)).collect())
    }
}

/// Monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub poll_interval_ms: u64,
    pub monitored_addresses: Option<Vec<String>>,
    pub block_callback: Option<fn(BlockEvent)>,
    pub transaction_callback: Option<fn(BlockEvent)>,
    pub address_callback: Option<fn(BlockEvent)>,
    pub runestone_callback: Option<fn(BlockEvent)>,
    pub alkanes_callback: Option<fn(BlockEvent)>,
    pub protorunes_callback: Option<fn(BlockEvent)>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 5000, // 5 seconds
            monitored_addresses: None,
            block_callback: None,
            transaction_callback: None,
            address_callback: None,
            runestone_callback: None,
            alkanes_callback: None,
            protorunes_callback: None,
        }
    }
}



/// Block event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub event_type: String,
    pub block_height: u64,
    pub txid: String,
    pub data: JsonValue,
}

/// Monitor statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorStats {
    pub is_running: bool,
    pub current_height: u64,
    pub blocks_processed: u64,
    pub events_processed: u64,
    pub uptime_seconds: u64,
    pub start_time: Option<SystemTime>,
}

/// Address monitor for tracking specific addresses
pub struct AddressMonitor<P: DeezelProvider> {
    provider: P,
    addresses: HashMap<String, AddressMonitorInfo>,
}

impl<P: DeezelProvider> AddressMonitor<P> {
    /// Create a new address monitor
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            addresses: HashMap::new(),
        }
    }
    
    /// Add address to monitor
    pub fn add_address(&mut self, address: String, label: Option<String>) {
        self.addresses.insert(address.clone(), AddressMonitorInfo {
            address: address.clone(),
            label,
            first_seen: SystemTime::now(),
            last_activity: None,
            transaction_count: 0,
            total_received: 0,
            total_sent: 0,
        });
    }
    
    /// Remove address from monitoring
    pub fn remove_address(&mut self, address: &str) {
        self.addresses.remove(address);
    }
    
    /// Check for new transactions on monitored addresses
    pub async fn check_address_activity(&mut self) -> Result<Vec<AddressActivity>> {
        let mut activities = Vec::new();
        
        for (address, info) in &mut self.addresses {
            match self.provider.get_address_txs(address).await {
                Ok(txs) => {
                    if let Some(txs_array) = txs.as_array() {
                        let new_tx_count = txs_array.len();
                        if new_tx_count > info.transaction_count {
                            // New transactions found
                            let new_txs = new_tx_count - info.transaction_count;
                            activities.push(AddressActivity {
                                address: address.clone(),
                                activity_type: "new_transactions".to_string(),
                                count: new_txs,
                                data: txs.clone(),
                            });
                            
                            info.transaction_count = new_tx_count;
                            info.last_activity = Some(SystemTime::now());
                        }
                    }
                }
                Err(e) => {
                    self.provider.warn(&format!("Failed to check activity for address {address}: {e}"));
                }
            }
        }
        
        Ok(activities)
    }
    
    /// Get address information
    pub fn get_address_info(&self, address: &str) -> Option<&AddressMonitorInfo> {
        self.addresses.get(address)
    }
    
    /// List all monitored addresses
    pub fn list_addresses(&self) -> Vec<&AddressMonitorInfo> {
        self.addresses.values().collect()
    }
}

/// Address monitor information
#[derive(Debug, Clone)]
pub struct AddressMonitorInfo {
    pub address: String,
    pub label: Option<String>,
    pub first_seen: SystemTime,
    pub last_activity: Option<SystemTime>,
    pub transaction_count: usize,
    pub total_received: u64,
    pub total_sent: u64,
}

/// Address activity
#[derive(Debug, Clone)]
pub struct AddressActivity {
    pub address: String,
    pub activity_type: String,
    pub count: usize,
    pub data: JsonValue,
}

/// Event filter for selective monitoring
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub event_types: Vec<String>,
    pub addresses: Vec<String>,
    pub min_amount: Option<u64>,
    pub max_amount: Option<u64>,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFilter {
    /// Create a new event filter
    pub fn new() -> Self {
        Self {
            event_types: Vec::new(),
            addresses: Vec::new(),
            min_amount: None,
            max_amount: None,
        }
    }
    
    /// Add event type to filter
    pub fn add_event_type(&mut self, event_type: String) {
        if !self.event_types.contains(&event_type) {
            self.event_types.push(event_type);
        }
    }
    
    /// Add address to filter
    pub fn add_address(&mut self, address: String) {
        if !self.addresses.contains(&address) {
            self.addresses.push(address);
        }
    }
    
    /// Set amount range
    pub fn set_amount_range(&mut self, min: Option<u64>, max: Option<u64>) {
        self.min_amount = min;
        self.max_amount = max;
    }
    
    /// Check if event matches filter
    pub fn matches(&self, event: &BlockEvent) -> bool {
        // Check event type
        if !self.event_types.is_empty() && !self.event_types.contains(&event.event_type) {
            return false;
        }
        
        // Check addresses (would need to extract addresses from event data)
        if !self.addresses.is_empty() {
            // This would check if any of the addresses in the event match the filter
            // For now, return true as a placeholder
        }
        
        // Check amount range (would need to extract amount from event data)
        if let (Some(min), Some(amount)) = (self.min_amount, event.data.get("amount").and_then(|v| v.as_u64())) {
            if amount < min {
                return false;
            }
        }
        
        if let (Some(max), Some(amount)) = (self.max_amount, event.data.get("amount").and_then(|v| v.as_u64())) {
            if amount > max {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_config() {
        let config = MonitorConfig::default();
        assert_eq!(config.poll_interval_ms, 5000);
        assert!(config.monitored_addresses.is_none());
    }
    
    
    #[test]
    fn test_event_filter() {
        let mut filter = EventFilter::new();
        filter.add_event_type("transaction".to_string());
        filter.add_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string());
        filter.set_amount_range(Some(1000), Some(100000));
        
        let event = BlockEvent {
            event_type: "transaction".to_string(),
            block_height: 800000,
            txid: "test_txid".to_string(),
            data: serde_json::json!({
                "amount": 50000
            }),
        };
        
        assert!(filter.matches(&event));
        
        let event2 = BlockEvent {
            event_type: "runestone".to_string(),
            block_height: 800000,
            txid: "test_txid".to_string(),
            data: serde_json::json!({}),
        };
        
        assert!(!filter.matches(&event2));
    }
    
    #[test]
    fn test_address_monitor_info() {
        let info = AddressMonitorInfo {
            address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
            label: Some("Test Address".to_string()),
            first_seen: SystemTime::now(),
            last_activity: None,
            transaction_count: 0,
            total_received: 0,
            total_sent: 0,
        };
        
        assert_eq!(info.address, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
        assert_eq!(info.label, Some("Test Address".to_string()));
        assert_eq!(info.transaction_count, 0);
    }
}