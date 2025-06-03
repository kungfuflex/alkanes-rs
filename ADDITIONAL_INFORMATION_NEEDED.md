# Additional Information Needed for OYL Integration

This document outlines specific information that would significantly enhance the OYL protocol integration implementation. The information is categorized by implementation area and priority.

## High Priority Information

### 1. OYL Protocol Storage Patterns

**What we need to know:**
- How does the OYL protocol store token metadata in alkane storage?
- What storage keys are used for pool information?
- How are holder balances tracked and stored?
- What is the storage layout for activity/transaction history?

**Why it's important:**
- Direct storage access via IndexPointer requires knowing exact storage keys
- Efficient queries depend on understanding the data organization
- Avoiding duplicate data storage and ensuring consistency

**Current assumptions:**
- Following standard alkane storage patterns with `/alkanes/{alkane_id}/` prefix
- Using protobuf serialization for complex data structures
- Implementing our own indexing tables for fast lookups

### 2. OYL Alkane Opcode Mapping

**What we need to know:**
- What opcodes are available in OYL factory contracts?
- What opcodes are available in OYL pool contracts?
- What parameters do these opcodes expect?
- What data do they return?

**Why it's important:**
- View functions need to call specific opcodes to get real-time data
- Parameter encoding must match what the alkanes expect
- Response parsing depends on understanding return formats

**Current assumptions:**
- Following patterns from reference implementation (opcodes 0-999)
- Using standard parameter encoding (u128 values)
- Implementing error handling for failed opcode calls

### 3. Price Oracle Integration Strategy

**What we need to know:**
- How should token prices be calculated from pool data?
- Are there external price feeds to integrate with?
- How should price history be stored and retrieved?
- What's the fallback strategy when pool prices are unavailable?

**Why it's important:**
- Price data is central to many OYL features (market cap, TVL, etc.)
- Historical price data enables price change calculations
- Accurate pricing affects all financial calculations

**Current assumptions:**
- Calculate prices from pool reserves using constant product formula
- Store price snapshots at regular intervals
- Implement weighted average pricing across multiple pools

### 4. Activity Event Structure

**What we need to know:**
- How are swap events structured in OYL transactions?
- What data is available for transfer events?
- How are mint/burn events recorded?
- What transaction metadata is accessible?

**Why it's important:**
- Activity feeds are a core feature requiring event parsing
- Transaction history affects volume and analytics calculations
- Event filtering and pagination requirements

**Current assumptions:**
- Parse events from alkane transaction traces
- Store events in chronological order with indexing
- Implement event type filtering and address-based queries

## Medium Priority Information

### 5. Performance and Scaling Requirements

**What we need to know:**
- Expected query volume and response time requirements
- Maximum result set sizes for different query types
- Caching strategy preferences
- Data retention policies

**Why it's important:**
- Determines indexing and caching strategies
- Affects storage design decisions
- Influences pagination implementation

### 6. Display and Formatting Preferences

**What we need to know:**
- Preferred number formatting (decimals, scientific notation)
- Date/time format preferences (ISO 8601, Unix timestamps)
- Unit conversion requirements (BTC/SATS/USD toggle implementation)
- Logo/image handling strategy

**Why it's important:**
- Consistent user experience across different interfaces
- Proper data presentation in client applications
- Standardized API responses

### 7. Integration Testing Strategy

**What we need to know:**
- Available test data or scenarios
- Specific OYL protocol deployments to test against
- Performance benchmarks or targets
- Compatibility requirements with existing tools

**Why it's important:**
- Ensures robust implementation
- Validates against real-world usage patterns
- Maintains compatibility with existing ecosystem

## Low Priority Information

### 8. Future Extension Plans

**What we need to know:**
- Planned OYL protocol updates or changes
- Additional features that might be added later
- Integration with other protocols or systems
- Versioning and backward compatibility requirements

**Why it's important:**
- Influences architectural decisions
- Ensures extensibility for future features
- Avoids technical debt

### 9. Deployment and Operations

**What we need to know:**
- Deployment environment requirements
- Monitoring and logging preferences
- Error reporting and alerting needs
- Maintenance and update procedures

**Why it's important:**
- Ensures smooth production deployment
- Enables effective monitoring and debugging
- Supports long-term maintenance

## Specific Implementation Questions

### Storage Layout Questions

1. **Token Storage**: 
   - Are token metadata stored in the token alkane's storage directly?
   - How are holder lists maintained and updated?
   - What's the key structure for holder balance lookups?

2. **Pool Storage**:
   - How are pool reserves tracked in real-time?
   - Where is liquidity provider information stored?
   - How are fee accumulation records maintained?

3. **Activity Storage**:
   - Are events stored in alkane storage or indexed separately?
   - What's the event ordering and pagination strategy?
   - How are cross-token activities linked?

### Opcode Interface Questions

1. **Factory Opcodes**:
   - What opcode returns all pools for a token pair?
   - How to get total number of pools created?
   - What data is returned for pool creation events?

2. **Pool Opcodes**:
   - What opcode returns current pool reserves?
   - How to get pool fee information?
   - What opcode provides liquidity provider data?

3. **Token Opcodes**:
   - What opcode returns token metadata (name, symbol, supply)?
   - How to get holder count and distribution?
   - What opcode provides transfer history?

### Price Calculation Questions

1. **Pool Pricing**:
   - Should we use time-weighted average prices (TWAP)?
   - How to handle low-liquidity pools?
   - What's the preferred price aggregation method across pools?

2. **Historical Data**:
   - What granularity for price history (hourly, daily)?
   - How far back should historical data go?
   - Should we store OHLCV data or just snapshots?

## Information Gathering Strategy

### 1. Code Analysis
- **Reference Implementation**: Deep dive into `reference/oyl-protocol/` alkanes
- **Storage Inspection**: Examine existing alkane storage patterns
- **Opcode Documentation**: Create comprehensive opcode mapping

### 2. Testing and Experimentation
- **Local Testing**: Deploy reference OYL contracts and test interactions
- **Storage Exploration**: Use existing view functions to understand data layout
- **Performance Testing**: Measure query performance with sample data

### 3. Documentation Review
- **OYL Protocol Docs**: Review any available protocol documentation
- **Alkanes Patterns**: Study existing alkane implementation patterns
- **Best Practices**: Research DeFi data indexing best practices

## Implementation Without Additional Information

Even without all the additional information, we can proceed with the implementation using reasonable assumptions:

1. **Start with Protobuf Schemas**: Define data structures based on requirements
2. **Implement Basic Storage**: Use standard alkane storage patterns
3. **Create View Function Stubs**: Implement basic query interfaces
4. **Add Opcode Integration**: Use simulate function to call alkane opcodes
5. **Iterative Refinement**: Improve implementation as more information becomes available

The modular design ensures that we can refine and optimize specific components as we gather more detailed information about the OYL protocol implementation.

## Next Steps for Information Gathering

1. **Analyze Reference Implementation**: Deep dive into OYL protocol alkanes
2. **Create Test Environment**: Set up local testing with OYL contracts
3. **Document Findings**: Create detailed documentation of discovered patterns
4. **Validate Assumptions**: Test our assumptions against real implementations
5. **Refine Implementation**: Update the implementation based on findings

This approach allows us to make progress while continuously improving the implementation quality as we learn more about the OYL protocol specifics.