//! Test configuration and utilities for deezel-common tests

use deezel_common::*;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment
pub fn init_test_env() {
    INIT.call_once(|| {
        // Initialize logging for tests
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init()
            .ok();
    });
}

/// Test configuration for different scenarios
pub struct TestConfig {
    pub network: bitcoin::Network,
    pub enable_logging: bool,
    pub mock_responses: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            network: bitcoin::Network::Regtest,
            enable_logging: true,
            mock_responses: true,
        }
    }
}

impl TestConfig {
    pub fn mainnet() -> Self {
        Self {
            network: bitcoin::Network::Bitcoin,
            ..Default::default()
        }
    }
    
    pub fn testnet() -> Self {
        Self {
            network: bitcoin::Network::Testnet,
            ..Default::default()
        }
    }
    
    pub fn with_real_responses(mut self) -> Self {
        self.mock_responses = false;
        self
    }
    
    pub fn without_logging(mut self) -> Self {
        self.enable_logging = false;
        self
    }
}

/// Test utilities
pub mod test_utils {
    use super::*;
    
    /// Generate test wallet configuration
    pub fn test_wallet_config(network: bitcoin::Network) -> WalletConfig {
        WalletConfig {
            wallet_path: format!("test_wallet_{:?}", network),
            network,
            bitcoin_rpc_url: match network {
                bitcoin::Network::Bitcoin => "http://localhost:8332".to_string(),
                bitcoin::Network::Testnet => "http://localhost:18332".to_string(),
                bitcoin::Network::Signet => "http://localhost:38332".to_string(),
                bitcoin::Network::Regtest => "http://localhost:18443".to_string(),
                _ => "http://localhost:18443".to_string(),
            },
            metashrew_rpc_url: "http://localhost:8080".to_string(),
            network_params: None,
        }
    }
    
    /// Generate test alkanes execute parameters
    pub fn test_alkanes_params() -> AlkanesExecuteParams {
        AlkanesExecuteParams {
            inputs: Some("B:100000".to_string()),
            to: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
            change: None,
            fee_rate: Some(10.0),
            envelope: None,
            protostones: "1:test_message".to_string(),
            trace: false,
            mine: false,
            auto_confirm: false,
            rebar: false,
        }
    }
    
    /// Generate test send parameters
    pub fn test_send_params() -> SendParams {
        SendParams {
            address: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
            amount: 100000,
            fee_rate: Some(10.0),
            send_all: false,
            from_address: None,
            change_address: None,
            auto_confirm: false,
        }
    }
    
    /// Generate test alkanes inspect config
    pub fn test_inspect_config() -> AlkanesInspectConfig {
        AlkanesInspectConfig {
            disasm: true,
            fuzz: true,
            fuzz_ranges: Some("0-100".to_string()),
            meta: true,
            codehash: true,
        }
    }
    
    /// Validate test results
    pub fn assert_valid_txid(txid: &str) {
        assert_eq!(txid.len(), 64, "TXID should be 64 characters");
        assert!(txid.chars().all(|c| c.is_ascii_hexdigit()), "TXID should be hex");
    }
    
    pub fn assert_valid_address(address: &str) {
        assert!(!address.is_empty(), "Address should not be empty");
        // Basic validation - in real tests we'd use proper address validation
        assert!(address.starts_with("bc1") || address.starts_with("tb1") || address.starts_with("bcrt1") || address.starts_with("1") || address.starts_with("3"));
    }
    
    pub fn assert_valid_balance(balance: &WalletBalance) {
        assert!(balance.confirmed <= balance.confirmed + balance.trusted_pending + balance.untrusted_pending);
    }
}

/// Performance test utilities
pub mod perf_utils {
    use std::time::{Duration, Instant};
    
    /// Measure execution time of an async function
    pub async fn measure_async<F, Fut, T>(f: F) -> (T, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }
    
    /// Assert that operation completes within expected time
    pub fn assert_performance<T>(result: (T, Duration), max_duration: Duration, operation: &str) -> T {
        let (value, actual_duration) = result;
        assert!(
            actual_duration <= max_duration,
            "{} took {:?}, expected <= {:?}",
            operation,
            actual_duration,
            max_duration
        );
        value
    }
    
    /// Performance benchmarks for key operations
    pub struct PerformanceBenchmarks {
        pub wallet_balance_max: Duration,
        pub address_generation_max: Duration,
        pub transaction_creation_max: Duration,
        pub alkanes_execution_max: Duration,
        pub runestone_analysis_max: Duration,
    }
    
    impl Default for PerformanceBenchmarks {
        fn default() -> Self {
            Self {
                wallet_balance_max: Duration::from_millis(100),
                address_generation_max: Duration::from_millis(50),
                transaction_creation_max: Duration::from_millis(500),
                alkanes_execution_max: Duration::from_secs(5),
                runestone_analysis_max: Duration::from_millis(200),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_config_creation() {
        let config = TestConfig::default();
        assert_eq!(config.network, bitcoin::Network::Regtest);
        assert!(config.enable_logging);
        assert!(config.mock_responses);
        
        let mainnet_config = TestConfig::mainnet();
        assert_eq!(mainnet_config.network, bitcoin::Network::Bitcoin);
    }
    
    #[test]
    fn test_wallet_config_generation() {
        let config = test_utils::test_wallet_config(bitcoin::Network::Bitcoin);
        assert_eq!(config.network, bitcoin::Network::Bitcoin);
        assert_eq!(config.bitcoin_rpc_url, "http://localhost:8332");
        
        let testnet_config = test_utils::test_wallet_config(bitcoin::Network::Testnet);
        assert_eq!(testnet_config.bitcoin_rpc_url, "http://localhost:18332");
    }
    
    #[test]
    fn test_validation_utils() {
        test_utils::assert_valid_txid("a1b2c3d4e5f67890123456789012345678901234567890123456789012345678");
        
        test_utils::assert_valid_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
        test_utils::assert_valid_address("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2");
        
        let balance = WalletBalance {
            confirmed: 100000,
            trusted_pending: 50000,
            untrusted_pending: 25000,
        };
        test_utils::assert_valid_balance(&balance);
    }
    
    #[tokio::test]
    async fn test_performance_measurement() {
        let (result, duration) = perf_utils::measure_async(|| async {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            42
        }).await;
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
        assert!(duration < Duration::from_millis(100)); // Should be much faster than 100ms
    }
}