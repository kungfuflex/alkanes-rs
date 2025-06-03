//! OYL Protocol View Functions
//! 
//! This module implements the main view functions for querying OYL protocol data.
//! These functions provide comprehensive access to token information, pool data,
//! positions, activities, and analytics.

use alkanes_support::id::AlkaneId;
use alkanes_support::proto::oyl::*;
use crate::oyl::{
    OylError, OylResult, constants::*,
    storage::{StorageUtils, PaginationUtils, *},
    utils::{TokenUtils, PoolUtils, PriceUtils, TimeUtils, ValidationUtils, ConversionUtils}
};
use metashrew_support::index_pointer::KeyValuePointer;
use protobuf::{Message, MessageField};
use std::collections::HashMap;

/// Token view functions
pub struct TokenView;

impl TokenView {
    /// Get comprehensive token information
    pub fn get_token_info(request: &TokenInfoRequest) -> OylResult<TokenInfoResponse> {
        let token_id = ConversionUtils::token_id_to_alkane_id(
            request.token_id.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing token_id".to_string()))?
        )?;
        
        ValidationUtils::validate_alkane_id(&token_id)?;
        
        let mut response = TokenInfoResponse::new();
        
        // Get basic token information
        let token_info = Self::build_token_info(&token_id)?;
        response.token = MessageField::some(token_info);
        
        // Get metrics if requested
        if request.include_metrics {
            let metrics = Self::build_token_metrics(&token_id)?;
            response.metrics = MessageField::some(metrics);
        }
        
        // Get price if requested
        if request.include_price {
            let price = Self::build_token_price(&token_id)?;
            response.price = MessageField::some(price);
        }
        
        Ok(response)
    }
    
    /// Get tokens held by a specific address
    pub fn get_tokens_by_holder(request: &TokensByHolderRequest) -> OylResult<TokensByHolderResponse> {
        ValidationUtils::validate_address(&request.wallet_address)?;
        let start_index = ValidationUtils::validate_pagination(
            request.limit.max(1).min(MAX_PAGE_SIZE), 
            &request.cursor
        )?;
        
        let mut response = TokensByHolderResponse::new();
        
        // Get tokens held by this address
        let address_key = StorageUtils::address_to_key(&request.wallet_address);
        let token_list = TOKENS_BY_HOLDER.select(&address_key).get_list();
        
        let mut holdings = Vec::new();
        let limit = request.limit as usize;
        let end_index = std::cmp::min(start_index as usize + limit, token_list.len());
        
        for i in (start_index as usize)..end_index {
            if let Some(token_bytes) = token_list.get(i) {
                let token_id = AlkaneId::try_from(token_bytes.as_ref().clone())
                    .map_err(|_| OylError::StorageError("Invalid token ID in storage".to_string()))?;
                
                // Get balance for this token
                let balance_key = StorageUtils::compound_key(&[
                    &StorageUtils::alkane_id_to_key(&token_id),
                    &address_key
                ]);
                let balance = TOKEN_HOLDERS.select(&balance_key).get_value::<u128>();
                
                if balance > 0 || request.include_zero_balances {
                    let holder = Self::build_token_holder(&request.wallet_address, &token_id, balance)?;
                    holdings.push(holder);
                }
            }
        }
        
        response.holdings = holdings;
        
        // Set pagination info
        let (has_more, next_cursor) = PaginationUtils::create_pagination_info(
            start_index, 
            request.limit, 
            token_list.len() as u64
        );
        response.has_more = has_more;
        response.next_cursor = next_cursor;
        
        Ok(response)
    }
    
    /// Get holders of a specific token
    pub fn get_token_holders(request: &TokenHoldersRequest) -> OylResult<TokenHoldersResponse> {
        let token_id = ConversionUtils::token_id_to_alkane_id(
            request.token_id.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing token_id".to_string()))?
        )?;
        
        ValidationUtils::validate_alkane_id(&token_id)?;
        let start_index = ValidationUtils::validate_pagination(request.limit, &request.cursor)?;
        
        let mut response = TokenHoldersResponse::new();
        
        // Get holders list for this token
        let token_key = StorageUtils::alkane_id_to_key(&token_id);
        let holders_list = HOLDERS_LIST.select(&token_key).get_list();
        
        let mut holders = Vec::new();
        let limit = request.limit as usize;
        let end_index = std::cmp::min(start_index as usize + limit, holders_list.len());
        
        for i in (start_index as usize)..end_index {
            if let Some(holder_bytes) = holders_list.get(i) {
                // Get balance for this holder
                let balance_key = StorageUtils::compound_key(&[&token_key, holder_bytes]);
                let balance = TOKEN_HOLDERS.select(&balance_key).get_value::<u128>();
                
                if balance >= request.min_balance.as_ref().map(|m| m.clone().into()).unwrap_or(0) {
                    let holder = Self::build_token_holder(holder_bytes, &token_id, balance)?;
                    holders.push(holder);
                }
            }
        }
        
        response.holders = holders;
        
        // Get total holder count
        let total_holders = HOLDER_COUNT.select(&token_key).get_value::<u64>();
        response.total_holders = total_holders;
        
        // Set pagination info
        let (has_more, next_cursor) = PaginationUtils::create_pagination_info(
            start_index, 
            request.limit, 
            holders_list.len() as u64
        );
        response.has_more = has_more;
        response.next_cursor = next_cursor;
        
        Ok(response)
    }
    
    /// Get price history for a token
    pub fn get_token_price_history(request: &TokenPriceHistoryRequest) -> OylResult<TokenPriceHistoryResponse> {
        let token_id = ConversionUtils::token_id_to_alkane_id(
            request.token_id.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing token_id".to_string()))?
        )?;
        
        ValidationUtils::validate_alkane_id(&token_id)?;
        
        let start_time = request.start_time.as_ref()
            .map(|t| t.seconds)
            .unwrap_or(0);
        let end_time = request.end_time.as_ref()
            .map(|t| t.seconds)
            .unwrap_or(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
        
        ValidationUtils::validate_time_range(start_time, end_time)?;
        
        let interval = if request.interval.is_empty() { "1h" } else { &request.interval };
        let bucket_size = match interval {
            "1h" => HOUR_SECONDS,
            "1d" => DAY_SECONDS,
            "1w" => WEEK_SECONDS,
            "1m" => MONTH_SECONDS,
            _ => return Err(OylError::InvalidRequest("Invalid interval".to_string())),
        };
        
        let mut response = TokenPriceHistoryResponse::new();
        let mut history = PriceHistory::new();
        
        // Get price data from storage
        let token_key = StorageUtils::alkane_id_to_key(&token_id);
        let price_data = StorageUtils::get_time_series_range(
            &*PRICE_HISTORY,
            &token_key,
            start_time,
            end_time,
            bucket_size
        )?;
        
        let mut points = Vec::new();
        for (timestamp, price_bytes) in price_data {
            if price_bytes.len() >= 8 {
                let price_value = f64::from_le_bytes(
                    price_bytes[0..8].try_into().unwrap_or([0u8; 8])
                );
                
                let mut point = PricePoint::new();
                let mut timestamp_msg = Timestamp::new();
                timestamp_msg.seconds = timestamp;
                point.timestamp = MessageField::some(timestamp_msg);
                
                let mut price = Price::new();
                price.value = price_value;
                price.currency = request.currency.clone();
                point.price = MessageField::some(price);
                
                points.push(point);
            }
        }
        
        history.token_id = request.token_id.clone();
        history.points = points;
        history.start_time = request.start_time.clone();
        history.end_time = request.end_time.clone();
        history.interval = interval.to_string();
        
        response.history = MessageField::some(history);
        
        Ok(response)
    }
    
    // Helper functions
    
    fn build_token_info(token_id: &AlkaneId) -> OylResult<TokenInfo> {
        let mut token_info = TokenInfo::new();
        
        // Set token ID
        token_info.id = MessageField::some(ConversionUtils::alkane_id_to_token_id(token_id));
        
        // Get metadata from opcode calls
        let (name, symbol, total_supply) = TokenUtils::get_token_metadata(token_id)?;
        token_info.name = name;
        token_info.symbol = symbol;
        token_info.total_supply = MessageField::some(total_supply.into());
        
        // Get additional info from storage if available
        let token_key = StorageUtils::alkane_id_to_key(token_id);
        let stored_info = TOKEN_INFO.select(&token_key).get();
        if !stored_info.is_empty() {
            // Parse stored token info if available
            if let Ok(stored) = TokenInfo::parse_from_bytes(&stored_info) {
                if !stored.logo_url.is_empty() {
                    token_info.logo_url = stored.logo_url;
                }
                if !stored.logo_data.is_empty() {
                    token_info.logo_data = stored.logo_data;
                }
                if stored.created_at.is_some() {
                    token_info.created_at = stored.created_at;
                }
                if stored.block_created != 0 {
                    token_info.block_created = stored.block_created;
                }
            }
        }
        
        Ok(token_info)
    }
    
    fn build_token_metrics(token_id: &AlkaneId) -> OylResult<TokenMetrics> {
        let mut metrics = TokenMetrics::new();
        
        metrics.token_id = MessageField::some(ConversionUtils::alkane_id_to_token_id(token_id));
        
        // Get holder count
        let token_key = StorageUtils::alkane_id_to_key(token_id);
        let holder_count = HOLDER_COUNT.select(&token_key).get_value::<u64>();
        metrics.total_holders = holder_count;
        
        // Get pool count
        let pools_list = POOLS_BY_TOKEN.select(&token_key).get_list();
        metrics.pool_count = pools_list.len() as u32;
        
        // Calculate volume, market cap, etc. from price and activity data
        // This would involve aggregating data from multiple sources
        
        Ok(metrics)
    }
    
    fn build_token_price(token_id: &AlkaneId) -> OylResult<TokenPrice> {
        let mut price = TokenPrice::new();
        
        price.token_id = MessageField::some(ConversionUtils::alkane_id_to_token_id(token_id));
        
        // Get current price from pools
        let current_price = Self::calculate_current_price(token_id)?;
        let mut price_msg = Price::new();
        price_msg.value = current_price;
        price_msg.currency = "USD".to_string();
        price.current_price = MessageField::some(price_msg);
        
        // Calculate price changes
        // This would involve comparing with historical prices
        
        Ok(price)
    }
    
    fn build_token_holder(address: &[u8], token_id: &AlkaneId, balance: u128) -> OylResult<TokenHolder> {
        let mut holder = TokenHolder::new();
        
        holder.address = address.to_vec();
        holder.balance = MessageField::some(balance.into());
        
        // Calculate percentage of supply
        let (_, _, total_supply) = TokenUtils::get_token_metadata(token_id)?;
        let percentage = TokenUtils::calculate_holder_percentage(balance, total_supply);
        let mut percent = Percentage::new();
        percent.value = percentage;
        holder.percent_of_supply = MessageField::some(percent);
        
        Ok(holder)
    }
    
    fn calculate_current_price(token_id: &AlkaneId) -> OylResult<f64> {
        // Get all pools containing this token
        let token_key = StorageUtils::alkane_id_to_key(token_id);
        let pools_list = POOLS_BY_TOKEN.select(&token_key).get_list();
        
        if pools_list.is_empty() {
            return Ok(0.0);
        }
        
        let mut pool_data = Vec::new();
        
        for pool_bytes in pools_list {
            if let Ok(pool_id) = AlkaneId::try_from(pool_bytes.as_ref().clone()) {
                if let Ok(details) = PoolUtils::get_pool_details(&pool_id) {
                    // Determine which reserve corresponds to our token
                    if details.token_a == *token_id {
                        pool_data.push((details.reserve_a, details.reserve_b, details.total_supply));
                    } else if details.token_b == *token_id {
                        pool_data.push((details.reserve_b, details.reserve_a, details.total_supply));
                    }
                }
            }
        }
        
        if pool_data.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate weighted average price
        PriceUtils::calculate_weighted_average_price(&pool_data)
    }
}

/// Pool view functions
pub struct PoolView;

impl PoolView {
    /// Get comprehensive pool information
    pub fn get_pool_info(request: &PoolInfoRequest) -> OylResult<PoolInfoResponse> {
        let pool_id = ConversionUtils::pool_id_to_alkane_id(
            request.pool_id.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing pool_id".to_string()))?
        )?;
        
        ValidationUtils::validate_alkane_id(&pool_id)?;
        
        let mut response = PoolInfoResponse::new();
        
        // Get pool information
        let pool_info = Self::build_pool_info(&pool_id)?;
        response.pool = MessageField::some(pool_info);
        
        // Get metrics if requested
        if request.include_metrics {
            let metrics = Self::build_pool_metrics(&pool_id)?;
            response.metrics = MessageField::some(metrics);
        }
        
        Ok(response)
    }
    
    /// Get pools containing a specific token
    pub fn get_pools_by_token(request: &PoolsByTokenRequest) -> OylResult<PoolsByTokenResponse> {
        let token_id = ConversionUtils::token_id_to_alkane_id(
            request.token_id.as_ref()
                .ok_or_else(|| OylError::InvalidRequest("Missing token_id".to_string()))?
        )?;
        
        ValidationUtils::validate_alkane_id(&token_id)?;
        let start_index = ValidationUtils::validate_pagination(request.limit, &request.cursor)?;
        
        let mut response = PoolsByTokenResponse::new();
        
        // Get pools containing this token
        let token_key = StorageUtils::alkane_id_to_key(&token_id);
        let pools_list = POOLS_BY_TOKEN.select(&token_key).get_list();
        
        let mut pools = Vec::new();
        let mut metrics = Vec::new();
        let limit = request.limit as usize;
        let end_index = std::cmp::min(start_index as usize + limit, pools_list.len());
        
        for i in (start_index as usize)..end_index {
            if let Some(pool_bytes) = pools_list.get(i) {
                if let Ok(pool_id) = AlkaneId::try_from(pool_bytes.as_ref().clone()) {
                    let pool_info = Self::build_pool_info(&pool_id)?;
                    pools.push(pool_info);
                    
                    if request.include_metrics {
                        let pool_metrics = Self::build_pool_metrics(&pool_id)?;
                        metrics.push(pool_metrics);
                    }
                }
            }
        }
        
        response.pools = pools;
        if request.include_metrics {
            response.metrics = metrics;
        }
        
        // Set pagination info
        let (has_more, next_cursor) = PaginationUtils::create_pagination_info(
            start_index, 
            request.limit, 
            pools_list.len() as u64
        );
        response.has_more = has_more;
        response.next_cursor = next_cursor;
        
        Ok(response)
    }
    
    /// Get all pools with optional sorting and filtering
    pub fn get_all_pools(request: &AllPoolsRequest) -> OylResult<AllPoolsResponse> {
        let start_index = ValidationUtils::validate_pagination(request.limit, &request.cursor)?;
        
        let mut response = AllPoolsResponse::new();
        
        // Get total pool count
        let total_pools = POOL_COUNT.get_value::<u64>();
        response.total_pools = total_pools;
        
        // Get pools with pagination
        let pools_data = StorageUtils::get_list_items(
            &*ALL_POOLS,
            b"",
            start_index,
            request.limit
        )?;
        
        let mut pools = Vec::new();
        let mut metrics = Vec::new();
        
        for pool_bytes in pools_data {
            if let Ok(pool_id) = AlkaneId::try_from(pool_bytes) {
                let pool_info = Self::build_pool_info(&pool_id)?;
                pools.push(pool_info);
                
                if request.include_metrics {
                    let pool_metrics = Self::build_pool_metrics(&pool_id)?;
                    metrics.push(pool_metrics);
                }
            }
        }
        
        // TODO: Implement sorting by TVL, volume, created_at
        
        response.pools = pools;
        if request.include_metrics {
            response.metrics = metrics;
        }
        
        // Set pagination info
        let (has_more, next_cursor) = PaginationUtils::create_pagination_info(
            start_index, 
            request.limit, 
            total_pools
        );
        response.has_more = has_more;
        response.next_cursor = next_cursor;
        
        Ok(response)
    }
    
    // Helper functions
    
    fn build_pool_info(pool_id: &AlkaneId) -> OylResult<PoolInfo> {
        let mut pool_info = PoolInfo::new();
        
        // Set pool ID
        pool_info.id = MessageField::some(ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        // Get pool details from opcode call
        let details = PoolUtils::get_pool_details(pool_id)?;
        
        pool_info.token_a = MessageField::some(ConversionUtils::alkane_id_to_token_id(&details.token_a));
        pool_info.token_b = MessageField::some(ConversionUtils::alkane_id_to_token_id(&details.token_b));
        pool_info.name = details.pool_name;
        pool_info.reserve_a = MessageField::some(details.reserve_a.into());
        pool_info.reserve_b = MessageField::some(details.reserve_b.into());
        pool_info.total_supply = MessageField::some(details.total_supply.into());
        
        // Get additional info from storage if available
        let pool_key = StorageUtils::alkane_id_to_key(pool_id);
        let stored_info = POOL_INFO.select(&pool_key).get();
        if !stored_info.is_empty() {
            if let Ok(stored) = PoolInfo::parse_from_bytes(&stored_info) {
                if stored.created_at.is_some() {
                    pool_info.created_at = stored.created_at;
                }
                if stored.block_created != 0 {
                    pool_info.block_created = stored.block_created;
                }
                if !stored.explorer_link.is_empty() {
                    pool_info.explorer_link = stored.explorer_link;
                }
            }
        }
        
        Ok(pool_info)
    }
    
    fn build_pool_metrics(pool_id: &AlkaneId) -> OylResult<PoolMetrics> {
        let mut metrics = PoolMetrics::new();
        
        metrics.pool_id = MessageField::some(ConversionUtils::alkane_id_to_pool_id(pool_id));
        
        // Calculate TVL, volume, APR, etc.
        // This would involve aggregating data from multiple sources
        
        Ok(metrics)
    }
}

// Additional view functions for positions, activities, and analytics would be implemented here
// Following the same patterns as above

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_info_request_validation() {
        let mut request = TokenInfoRequest::new();
        let result = TokenView::get_token_info(&request);
        assert!(result.is_err()); // Should fail due to missing token_id
    }
    
    #[test]
    fn test_pagination_validation() {
        let result = ValidationUtils::validate_pagination(0, &[]);
        assert!(result.is_err()); // Should fail due to invalid limit
        
        let result = ValidationUtils::validate_pagination(50, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}