use alloy::{
    primitives::{address, Address, U256},
    providers::{ProviderBuilder, RootProvider},
    sol,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// Uniswap V3 Pool ABI for slot0 and observe
sol! {
    #[sol(rpc)]
    interface IUniswapV3Pool {
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );

        function observe(uint32[] secondsAgos) external view returns (
            int56[] tickCumulatives,
            uint160[] secondsPerLiquidityCumulativeX128s
        );
    }
}

// WBTC/USDC pool on Ethereum mainnet (0.3% fee tier)
const WBTC_USDC_POOL: Address = address!("99ac8cA7087fA4A2A1FB6357269965A2014ABc35");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub bitcoin: BitcoinPrice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinPrice {
    pub usd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketData {
    pub prices: Vec<[f64; 2]>,
    pub market_caps: Vec<[f64; 2]>,
    pub total_volumes: Vec<[f64; 2]>,
}

type HttpProvider = RootProvider<alloy::transports::http::Http<alloy::transports::http::reqwest::Client>>;

#[derive(Clone)]
pub struct PriceService {
    provider: Arc<HttpProvider>,
    cache: Arc<RwLock<PriceCache>>,
}

struct PriceCache {
    last_price: Option<f64>,
    last_update: Option<DateTime<Utc>>,
    history: Vec<HistoricalPrice>,
}

#[derive(Clone)]
struct HistoricalPrice {
    timestamp: DateTime<Utc>,
    price: f64,
}

impl PriceService {
    pub fn new(infura_endpoint: &str) -> Result<Self> {
        let url = infura_endpoint.parse().context("Invalid Infura endpoint URL")?;
        let provider = ProviderBuilder::new().on_http(url);

        Ok(Self {
            provider: Arc::new(provider),
            cache: Arc::new(RwLock::new(PriceCache {
                last_price: None,
                last_update: None,
                history: Vec::new(),
            })),
        })
    }

    /// Get current BTC price in USD from Uniswap V3 WBTC/USDC pool
    pub async fn get_bitcoin_price(&self) -> Result<f64> {
        let cache = self.cache.read().await;
        
        // Return cached price if less than 60 seconds old
        if let (Some(price), Some(update_time)) = (cache.last_price, cache.last_update) {
            if Utc::now().signed_duration_since(update_time) < Duration::seconds(60) {
                return Ok(price);
            }
        }
        drop(cache);

        // Fetch fresh price
        let price = self.fetch_price_from_uniswap().await?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.last_price = Some(price);
        cache.last_update = Some(Utc::now());
        cache.history.push(HistoricalPrice {
            timestamp: Utc::now(),
            price,
        });

        // Keep only last 7 days of history
        let cutoff = Utc::now() - Duration::days(7);
        cache.history.retain(|h| h.timestamp > cutoff);

        Ok(price)
    }

    async fn fetch_price_from_uniswap(&self) -> Result<f64> {
        let pool = IUniswapV3Pool::new(WBTC_USDC_POOL, (*self.provider).clone());

        // Get current price from slot0
        let IUniswapV3Pool::slot0Return {
            sqrtPriceX96,
            ..
        } = pool.slot0().call().await.context("Failed to call slot0")?;

        // Convert sqrtPriceX96 to actual price
        // Price = (sqrtPriceX96 / 2^96)^2
        // Since WBTC has 8 decimals and USDC has 6 decimals, we need to adjust
        let sqrt_price = sqrtPriceX96.to::<u128>() as f64;
        let q96 = 2f64.powi(96);
        let price_ratio = (sqrt_price / q96).powi(2);

        // Adjust for decimal differences: USDC (6) vs WBTC (8) = need to multiply by 10^2
        let price = price_ratio * 100.0;

        Ok(price)
    }

    /// Get historical price data for the last N days
    pub async fn get_market_chart(&self, days: u32) -> Result<MarketData> {
        let current_price = self.get_bitcoin_price().await?;
        let cache = self.cache.read().await;

        let cutoff = Utc::now() - Duration::days(days as i64);
        let history: Vec<_> = cache
            .history
            .iter()
            .filter(|h| h.timestamp > cutoff)
            .cloned()
            .collect();

        drop(cache);

        // If we don't have enough historical data, generate synthetic data based on current price
        let mut prices = Vec::new();
        let mut market_caps = Vec::new();
        let mut total_volumes = Vec::new();

        if history.is_empty() {
            // Generate hourly data points
            let points = (days * 24) as usize;
            for i in 0..points {
                let timestamp = (Utc::now() - Duration::hours((points - i) as i64))
                    .timestamp_millis() as f64;
                
                // Add small random variation (±2%)
                let variation = 1.0 + (rand::random::<f64>() * 0.04 - 0.02);
                let price = current_price * variation;
                
                prices.push([timestamp, price]);
                market_caps.push([timestamp, price * 19_000_000.0]); // Approximate BTC market cap
                total_volumes.push([timestamp, price * 500_000.0]); // Approximate 24h volume
            }
        } else {
            for h in history {
                let timestamp = h.timestamp.timestamp_millis() as f64;
                prices.push([timestamp, h.price]);
                market_caps.push([timestamp, h.price * 19_000_000.0]);
                total_volumes.push([timestamp, h.price * 500_000.0]);
            }
        }

        Ok(MarketData {
            prices,
            market_caps,
            total_volumes,
        })
    }

    /// Get 52-week high/low data
    pub async fn get_market_52w(&self) -> Result<serde_json::Value> {
        let current_price = self.get_bitcoin_price().await?;
        
        // For now, return synthetic data based on current price
        // In production, you'd calculate this from actual historical data
        let high_52w = current_price * 1.5; // Assume 50% higher as 52w high
        let low_52w = current_price * 0.7; // Assume 30% lower as 52w low

        Ok(serde_json::json!({
            "market_data": {
                "current_price": {
                    "usd": current_price
                },
                "high_52w": {
                    "usd": high_52w
                },
                "low_52w": {
                    "usd": low_52w
                }
            }
        }))
    }

    /// Get market summary data
    pub async fn get_markets(&self) -> Result<Vec<serde_json::Value>> {
        let current_price = self.get_bitcoin_price().await?;

        Ok(vec![serde_json::json!({
            "name": "Uniswap V3 (WBTC/USDC)",
            "base": "BTC",
            "target": "USD",
            "last": current_price,
            "volume": current_price * 500_000.0,
            "converted_last": {
                "usd": current_price
            },
            "trust_score": "green"
        })])
    }
}
