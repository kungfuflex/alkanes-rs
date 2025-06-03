# Active Context - OYL Protocol Integration

## Current Focus

We are implementing an OYL (Oil) protocol integration into alkanes-rs that will provide comprehensive view functions for DeFi data points including tokens, pools, positions, and activities.

## Recent Progress

✅ **Reference Material Completed**: Created comprehensive OYL_REFERENCE_MATERIAL.md with:
- Complete OYL contract analysis (token, factory, pool contracts)
- Detailed opcode mapping and storage patterns
- Alkanes-rs storage access patterns
- Price calculation strategies
- Implementation recommendations

## Key Discoveries

### OYL Contract Structure
- **OYL Token**: Opcodes 0, 99, 100, 101, 1000 for initialization and metadata
- **AMM Factory**: Manages pool creation with storage at `/pools/`, `/all_pools/`
- **AMM Pool**: Handles swapping/liquidity with storage at `/factory_id`, `/alkane/0`, `/alkane/1`

### Storage Patterns
- **Global Layout**: `/alkanes/{alkane_id}/` for alkane-specific data
- **Balance Storage**: `/alkanes/{token_id}/balances/{holder_id}`
- **Inventory Tracking**: `/alkanes/{holder_id}/inventory/`
- **Internal Storage**: `/alkanes/{alkane_id}/storage/{key}`

### Data Access Methods
- **Opcode Calls**: Use `call_view()` with specific opcodes to get real-time data
- **Direct Storage**: Use `IndexPointer` to access stored data directly
- **Batch Operations**: Use `call_multiview()` for efficient multi-token queries

## Implementation Strategy

The implementation will follow these patterns:
1. **Protobuf Schemas**: Define all data structures in `crates/alkanes-support/proto/oyl.proto`
2. **Storage Tables**: Create efficient lookup tables for common queries
3. **View Functions**: Implement RPC-callable functions following existing patterns
4. **Price Calculation**: Use pool reserves with constant product formula
5. **Activity Tracking**: Extract events from transaction traces

## Next Steps

1. **Begin Implementation**: Start with Phase 1 (Protobuf Schema Definition)
2. **Create Storage Layer**: Implement efficient indexing tables
3. **Build View Functions**: Create comprehensive query interface
4. **Add Integration Points**: Connect to existing alkanes-rs infrastructure

## Current Status

- ✅ Project analysis complete
- ✅ Requirements documented  
- ✅ Action plan created
- ✅ Reference material compiled
- ⏳ Implementation ready to begin
- ⏸️ Testing pending
- ⏸️ Integration pending

## Available Resources

- **OYL_ACTION_PLAN.md**: Complete 6-phase implementation roadmap
- **OYL_REFERENCE_MATERIAL.md**: Comprehensive technical reference
- **ADDITIONAL_INFORMATION_NEEDED.md**: Enhancement opportunities
- **Memory Bank**: Project context and patterns documentation

The foundation is now complete for implementing the OYL protocol integration with all necessary reference material and implementation guidance.