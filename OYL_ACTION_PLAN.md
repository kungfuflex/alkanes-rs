# OYL Protocol Integration Action Plan

## Overview

This document outlines the complete action plan for integrating OYL (Oil) protocol functionality into alkanes-rs. The integration will provide comprehensive view functions with protobuf schemas to capture DeFi data points for tokens, pools, positions, and activities.

## Implementation Phases

### Phase 1: Protobuf Schema Definition

**Objective**: Define all protobuf message types for OYL data structures

**Files to Create/Modify**:
- `crates/alkanes-support/proto/oyl.proto` - New protobuf definitions
- `crates/alkanes-support/build.rs` - Add oyl.proto to build process
- `crates/alkanes-support/src/proto/mod.rs` - Export oyl module

**Protobuf Messages to Define**:

```protobuf
// Token-related messages
message TokenInfo { ... }
message TokenPrice { ... }
message TokenActivity { ... }
message TokenHolder { ... }

// Pool-related messages  
message PoolInfo { ... }
message PoolBalance { ... }
message PoolTransaction { ... }

// Position-related messages
message Position { ... }
message PositionFees { ... }

// Activity-related messages
message ActivityEvent { ... }
message ActivityFeed { ... }

// Request/Response wrappers
message TokenRequest { ... }
message TokenResponse { ... }
message PoolRequest { ... }
message PoolResponse { ... }
// ... etc
```

**Estimated Time**: 2-3 days

### Phase 2: Storage Layer Implementation

**Objective**: Implement storage access patterns for OYL data using IndexPointer

**Files to Create/Modify**:
- `src/oyl/storage.rs` - Storage access patterns
- `src/oyl/tables.rs` - Table definitions for OYL data

**Storage Tables to Implement**:
```rust
// Token storage
pub static TOKEN_INFO: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/tokens/"));
pub static TOKEN_HOLDERS: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/token_holders/"));
pub static TOKEN_PRICES: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/prices/"));

// Pool storage
pub static POOL_INFO: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/pools/"));
pub static POOL_BALANCES: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/pool_balances/"));

// Position storage
pub static POSITIONS: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/positions/"));
pub static POSITIONS_BY_ADDRESS: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/positions_by_address/"));

// Activity storage
pub static ACTIVITIES: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/activities/"));
pub static ACTIVITIES_BY_TOKEN: LazyLock<KeyValuePointer> = LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/activities_by_token/"));
```

**Estimated Time**: 3-4 days

### Phase 3: Core View Functions Implementation

**Objective**: Implement the main view functions for querying OYL data

**Files to Create**:
- `src/oyl/mod.rs` - Main module file
- `src/oyl/view.rs` - View function implementations
- `src/oyl/utils.rs` - Utility functions

**View Functions to Implement**:

```rust
// Token view functions
pub fn token_by_id(input: &Vec<u8>) -> Result<TokenResponse>;
pub fn tokens_by_holder(input: &Vec<u8>) -> Result<TokensResponse>;
pub fn token_activity(input: &Vec<u8>) -> Result<ActivityResponse>;
pub fn token_price_history(input: &Vec<u8>) -> Result<PriceHistoryResponse>;

// Pool view functions
pub fn pool_by_id(input: &Vec<u8>) -> Result<PoolResponse>;
pub fn pools_by_token(input: &Vec<u8>) -> Result<PoolsResponse>;
pub fn pool_transactions(input: &Vec<u8>) -> Result<TransactionsResponse>;

// Position view functions
pub fn positions_by_address(input: &Vec<u8>) -> Result<PositionsResponse>;
pub fn position_by_pool(input: &Vec<u8>) -> Result<PositionResponse>;

// Activity view functions
pub fn activity_by_token(input: &Vec<u8>) -> Result<ActivityResponse>;
pub fn activity_by_address(input: &Vec<u8>) -> Result<ActivityResponse>;
```

**Implementation Pattern** (following existing patterns):
```rust
pub fn token_by_id(input: &Vec<u8>) -> Result<TokenResponse> {
    let request = TokenRequest::parse_from_bytes(input)?;
    let mut response = TokenResponse::new();
    
    // Access storage using IndexPointer
    let token_data = TOKEN_INFO.select(&request.token_id).get();
    if !token_data.is_empty() {
        let token_info = TokenInfo::parse_from_bytes(&token_data)?;
        response.token = MessageField::some(token_info);
    }
    
    // Call alkane opcodes if needed using simulate
    if let Some(alkane_id) = get_token_alkane_id(&request.token_id) {
        let price_data = call_view(&alkane_id, &vec![PRICE_OPCODE], STATIC_FUEL)?;
        // Process price data...
    }
    
    Ok(response)
}
```

**Estimated Time**: 5-6 days

### Phase 4: Integration with Alkanes Runtime

**Objective**: Integrate OYL view functions with the existing alkanes runtime

**Files to Modify**:
- `src/lib.rs` - Add OYL module and export functions
- `Cargo.toml` - Add "oyl" feature flag

**Integration Steps**:

1. **Add Module to lib.rs**:
```rust
#[cfg(feature = "oyl")]
pub mod oyl;
```

2. **Add Feature Flag to Cargo.toml**:
```toml
[features]
oyl = []
```

3. **Add RPC Export Functions**:
```rust
#[cfg(all(not(test), feature = "oyl"))]
#[no_mangle]
pub fn oyl_token_by_id() -> i32 {
    configure_network();
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result = oyl::view::token_by_id(&consume_to_end(&mut data).unwrap())
        .unwrap_or_else(|_| oyl::proto::TokenResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(all(not(test), feature = "oyl"))]
#[no_mangle]
pub fn oyl_pool_by_id() -> i32 {
    // Similar implementation...
}

// Add more RPC functions...
```

**Estimated Time**: 2-3 days

### Phase 5: Data Population and Indexing

**Objective**: Implement data population during block indexing

**Files to Create/Modify**:
- `src/oyl/indexer.rs` - OYL-specific indexing logic
- `src/indexer.rs` - Integrate OYL indexing

**Indexing Integration**:
```rust
// In src/indexer.rs
#[cfg(feature = "oyl")]
use crate::oyl::indexer as oyl_indexer;

pub fn index_block(block: &Block, height: u32) -> Result<()> {
    // ... existing indexing logic ...
    
    #[cfg(feature = "oyl")]
    oyl_indexer::index_oyl_data(block, height)?;
    
    Ok(())
}
```

**Data Population Strategy**:
- Monitor alkane transactions for OYL protocol interactions
- Extract token creation, pool creation, swap events
- Calculate derived data (prices, TVL, volumes)
- Update storage tables with new data

**Estimated Time**: 4-5 days

### Phase 6: Testing and Validation

**Objective**: Comprehensive testing of OYL integration

**Files to Create**:
- `src/oyl/tests/mod.rs` - Test module
- `src/oyl/tests/token_tests.rs` - Token view function tests
- `src/oyl/tests/pool_tests.rs` - Pool view function tests
- `src/oyl/tests/integration_tests.rs` - Integration tests

**Test Categories**:
1. **Unit Tests**: Test individual view functions
2. **Integration Tests**: Test with reference OYL protocol alkanes
3. **Performance Tests**: Ensure efficient data access
4. **RPC Tests**: Test external RPC interface

**Estimated Time**: 3-4 days

## Additional Information Needed

To enhance the action plan and implementation, the following information would be valuable:

### 1. OYL Protocol Specifics
- **Storage Layout**: How does the OYL protocol store data in alkane storage?
- **Opcode Mapping**: What opcodes are used for different operations?
- **Event Structure**: How are swap/transfer events structured?

### 2. Price Oracle Integration
- **Price Sources**: How should prices be calculated/fetched?
- **Historical Data**: How to store and retrieve price history?
- **Fallback Mechanisms**: What to do when price data is unavailable?

### 3. Performance Requirements
- **Query Limits**: Are there limits on result set sizes?
- **Caching Strategy**: Should we implement caching for frequently accessed data?
- **Indexing Strategy**: What data needs to be indexed for fast queries?

### 4. Display Options
- **Unit Conversion**: Implementation details for BTC/SATS/USD conversion
- **Formatting**: How should numbers and dates be formatted?
- **Localization**: Any localization requirements?

## Implementation Timeline

**Total Estimated Time**: 19-25 days

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Protobuf Schema | 2-3 days | None |
| Phase 2: Storage Layer | 3-4 days | Phase 1 |
| Phase 3: View Functions | 5-6 days | Phase 1, 2 |
| Phase 4: Integration | 2-3 days | Phase 3 |
| Phase 5: Data Population | 4-5 days | Phase 2, 4 |
| Phase 6: Testing | 3-4 days | All previous phases |

## Risk Mitigation

### Technical Risks
- **Performance**: Large datasets may cause slow queries
  - *Mitigation*: Implement pagination and caching
- **Storage Growth**: OYL data may consume significant storage
  - *Mitigation*: Implement data retention policies
- **Compatibility**: Changes to OYL protocol may break integration
  - *Mitigation*: Version the protobuf schemas and maintain backward compatibility

### Implementation Risks
- **Complexity**: OYL integration adds significant complexity
  - *Mitigation*: Implement incrementally with thorough testing
- **Maintenance**: Additional code to maintain and debug
  - *Mitigation*: Comprehensive documentation and test coverage

## Success Criteria

1. **Functional**: All specified view functions work correctly
2. **Performance**: Queries complete within acceptable time limits
3. **Compatibility**: Integration doesn't break existing functionality
4. **Maintainable**: Code is well-documented and testable
5. **Extensible**: Easy to add new OYL features in the future

## Next Steps

1. **Gather Additional Information**: Collect the information listed in the "Additional Information Needed" section
2. **Start Phase 1**: Begin implementing protobuf schemas
3. **Set Up Development Environment**: Ensure all tools and dependencies are available
4. **Create Development Branch**: Set up version control for the OYL integration work

This action plan provides a comprehensive roadmap for implementing OYL protocol integration into alkanes-rs while maintaining the existing architecture patterns and ensuring robust, maintainable code.