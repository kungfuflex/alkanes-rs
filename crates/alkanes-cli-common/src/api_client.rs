use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Alkanes Data API Client
pub struct AlkanesApiClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl AlkanesApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::blocking::Client::new(),
        }
    }

    fn post<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<R> {
        let url = format!("{}/api/v1/{}", self.base_url, endpoint);
        let response = self.client.post(&url).json(body).send()?;
        
        if !response.status().is_success() {
            anyhow::bail!("API request failed: {}", response.status());
        }
        
        Ok(response.json()?)
    }

    // Balance endpoints
    pub fn get_address_balances(
        &self,
        address: &str,
        include_outpoints: bool,
    ) -> Result<AddressBalancesResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            address: &'a str,
            include_outpoints: bool,
        }

        self.post(
            "get-address-balances",
            &Request {
                address,
                include_outpoints,
            },
        )
    }

    pub fn get_outpoint_balances(&self, outpoint: &str) -> Result<OutpointBalancesResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            outpoint: &'a str,
        }

        self.post("get-outpoint-balances", &Request { outpoint })
    }

    pub fn get_holders(
        &self,
        alkane: &str,
        page: i64,
        limit: i64,
    ) -> Result<HoldersResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            alkane: &'a str,
            page: i64,
            limit: i64,
        }

        self.post("get-holders", &Request { alkane, page, limit })
    }

    pub fn get_holders_count(&self, alkane: &str) -> Result<HolderCountResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            alkane: &'a str,
        }

        self.post("get-holders-count", &Request { alkane })
    }

    pub fn get_address_outpoints(&self, address: &str) -> Result<AddressOutpointsResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            address: &'a str,
        }

        self.post("get-address-outpoints", &Request { address })
    }

    // Storage endpoints
    pub fn get_keys(
        &self,
        alkane: &str,
        prefix: Option<String>,
        limit: i64,
    ) -> Result<GetKeysResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            alkane: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            prefix: Option<String>,
            limit: i64,
        }

        self.post("get-keys", &Request { alkane, prefix, limit })
    }

    // AMM endpoints
    pub fn get_trades(
        &self,
        pool: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<GetTradesResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            pool: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            start_time: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            end_time: Option<i64>,
            limit: i64,
        }

        self.post(
            "get-trades",
            &Request {
                pool,
                start_time,
                end_time,
                limit,
            },
        )
    }

    pub fn get_candles(
        &self,
        pool: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<GetCandlesResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            pool: &'a str,
            interval: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            start_time: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            end_time: Option<i64>,
            limit: i64,
        }

        self.post(
            "get-candles",
            &Request {
                pool,
                interval,
                start_time,
                end_time,
                limit,
            },
        )
    }

    pub fn get_reserves(&self, pool: &str) -> Result<GetReservesResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            pool: &'a str,
        }

        self.post("get-reserves", &Request { pool })
    }

    pub fn pathfind(
        &self,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
        max_hops: i32,
    ) -> Result<PathfindResponse> {
        #[derive(Serialize)]
        struct Request<'a> {
            token_in: &'a str,
            token_out: &'a str,
            amount_in: &'a str,
            max_hops: i32,
        }

        self.post(
            "pathfind",
            &Request {
                token_in,
                token_out,
                amount_in,
                max_hops,
            },
        )
    }
}

// Response types
#[derive(Debug, Deserialize)]
pub struct AddressBalancesResponse {
    pub ok: bool,
    pub address: String,
    pub balances: HashMap<String, String>,
    pub outpoints: Option<Vec<OutpointInfo>>,
}

#[derive(Debug, Deserialize)]
pub struct OutpointInfo {
    pub outpoint: String,
    pub entries: Vec<BalanceEntry>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceEntry {
    pub alkane: String,
    pub amount: String,
}

#[derive(Debug, Deserialize)]
pub struct OutpointBalancesResponse {
    pub ok: bool,
    pub outpoint: String,
    pub items: Vec<OutpointItem>,
}

#[derive(Debug, Deserialize)]
pub struct OutpointItem {
    pub outpoint: String,
    pub address: Option<String>,
    pub entries: Vec<BalanceEntry>,
}

#[derive(Debug, Deserialize)]
pub struct HoldersResponse {
    pub ok: bool,
    pub alkane: String,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub has_more: bool,
    pub items: Vec<HolderInfo>,
}

#[derive(Debug, Deserialize)]
pub struct HolderInfo {
    pub address: String,
    pub amount: String,
}

#[derive(Debug, Deserialize)]
pub struct HolderCountResponse {
    pub ok: bool,
    pub alkane: String,
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddressOutpointsResponse {
    pub ok: bool,
    pub address: String,
    pub outpoints: Vec<OutpointInfo>,
}

#[derive(Debug, Deserialize)]
pub struct GetKeysResponse {
    pub ok: bool,
    pub alkane: String,
    pub keys: HashMap<String, KeyValue>,
}

#[derive(Debug, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub last_txid: String,
    pub last_vout: i32,
    pub block_height: i32,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GetTradesResponse {
    pub ok: bool,
    pub pool: String,
    pub trades: Vec<TradeInfo>,
}

#[derive(Debug, Deserialize)]
pub struct TradeInfo {
    pub txid: String,
    pub vout: i32,
    pub token0: String,
    pub token1: String,
    pub amount0_in: String,
    pub amount1_in: String,
    pub amount0_out: String,
    pub amount1_out: String,
    pub reserve0_after: String,
    pub reserve1_after: String,
    pub timestamp: String,
    pub block_height: i32,
}

#[derive(Debug, Deserialize)]
pub struct GetCandlesResponse {
    pub ok: bool,
    pub pool: String,
    pub interval: String,
    pub candles: Vec<CandleInfo>,
}

#[derive(Debug, Deserialize)]
pub struct CandleInfo {
    pub open_time: String,
    pub close_time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume0: String,
    pub volume1: String,
    pub trade_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct GetReservesResponse {
    pub ok: bool,
    pub pool: String,
    pub reserve0: String,
    pub reserve1: String,
    pub timestamp: String,
    pub block_height: i32,
}

#[derive(Debug, Deserialize)]
pub struct PathfindResponse {
    pub ok: bool,
    pub paths: Vec<PathInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PathInfo {
    pub hops: Vec<String>,
    pub pools: Vec<String>,
    pub estimated_output: String,
}
