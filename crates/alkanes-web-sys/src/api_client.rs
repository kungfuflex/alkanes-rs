use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[wasm_bindgen]
pub struct AlkanesWebApiClient {
    base_url: String,
}

#[wasm_bindgen]
impl AlkanesWebApiClient {
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    async fn post(&self, endpoint: &str, body: JsonValue) -> Result<JsonValue, JsValue> {
        let url = format!("{}/api/v1/{}", self.base_url, endpoint);
        
        let window = web_sys::window().ok_or("No window")?;
        let resp = JsFuture::from(
            window
                .fetch_with_str_and_init(
                    &url,
                    web_sys::RequestInit::new()
                        .method("POST")
                        .body(Some(&JsValue::from_str(&body.to_string()))),
                )
        )
        .await?;

        let resp: web_sys::Response = resp.dyn_into()?;
        let json = JsFuture::from(resp.json()?).await?;
        
        Ok(serde_wasm_bindgen::from_value(json)?)
    }

    #[wasm_bindgen(js_name = getAddressBalances)]
    pub async fn get_address_balances(
        &self,
        address: String,
        include_outpoints: bool,
    ) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "address": address,
            "include_outpoints": include_outpoints,
        });

        let result = self.post("get-address-balances", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getOutpointBalances)]
    pub async fn get_outpoint_balances(&self, outpoint: String) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "outpoint": outpoint,
        });

        let result = self.post("get-outpoint-balances", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getHolders)]
    pub async fn get_holders(
        &self,
        alkane: String,
        page: i64,
        limit: i64,
    ) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "alkane": alkane,
            "page": page,
            "limit": limit,
        });

        let result = self.post("get-holders", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getHoldersCount)]
    pub async fn get_holders_count(&self, alkane: String) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "alkane": alkane,
        });

        let result = self.post("get-holders-count", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getAddressOutpoints)]
    pub async fn get_address_outpoints(&self, address: String) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "address": address,
        });

        let result = self.post("get-address-outpoints", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getKeys)]
    pub async fn get_keys(
        &self,
        alkane: String,
        prefix: Option<String>,
        limit: i64,
    ) -> Result<JsValue, JsValue> {
        let mut body = serde_json::json!({
            "alkane": alkane,
            "limit": limit,
        });

        if let Some(p) = prefix {
            body["prefix"] = JsonValue::String(p);
        }

        let result = self.post("get-keys", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getTrades)]
    pub async fn get_trades(
        &self,
        pool: String,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<JsValue, JsValue> {
        let mut body = serde_json::json!({
            "pool": pool,
            "limit": limit,
        });

        if let Some(st) = start_time {
            body["start_time"] = JsonValue::from(st);
        }
        if let Some(et) = end_time {
            body["end_time"] = JsonValue::from(et);
        }

        let result = self.post("get-trades", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getCandles)]
    pub async fn get_candles(
        &self,
        pool: String,
        interval: String,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<JsValue, JsValue> {
        let mut body = serde_json::json!({
            "pool": pool,
            "interval": interval,
            "limit": limit,
        });

        if let Some(st) = start_time {
            body["start_time"] = JsonValue::from(st);
        }
        if let Some(et) = end_time {
            body["end_time"] = JsonValue::from(et);
        }

        let result = self.post("get-candles", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = getReserves)]
    pub async fn get_reserves(&self, pool: String) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "pool": pool,
        });

        let result = self.post("get-reserves", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen(js_name = pathfind)]
    pub async fn pathfind(
        &self,
        token_in: String,
        token_out: String,
        amount_in: String,
        max_hops: i32,
    ) -> Result<JsValue, JsValue> {
        let body = serde_json::json!({
            "token_in": token_in,
            "token_out": token_out,
            "amount_in": amount_in,
            "max_hops": max_hops,
        });

        let result = self.post("pathfind", body).await?;
        Ok(serde_wasm_bindgen::to_value(&result)?)
    }
}

use wasm_bindgen_futures::JsFuture;
