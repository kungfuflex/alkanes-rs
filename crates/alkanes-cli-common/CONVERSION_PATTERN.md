# Conversion Pattern Documentation

## Overview

This document describes the consolidated conversion pattern adopted from `subfrost-common` and integrated into `alkanes-cli-common`. The pattern solves the orphan rule problem when working with external protobuf types.

## Problem Statement

Rust's orphan rule (E0117) prevents implementing foreign traits (like `From`) for foreign types. When working with protobuf types from `alkanes-support`, we cannot directly implement `From<protobuf::Type>` for our domain types. This requires an alternative approach.

## Solution: Explicit Conversion Functions

Instead of relying on trait implementations, we provide explicit conversion functions in a dedicated `conversion` module. This approach:

1. **Avoids orphan rule violations** - We own the conversion functions
2. **Provides clear conversion paths** - Explicit function names make intent clear
3. **Enables bidirectional conversions** - Both `to_proto_*` and `convert_*` functions
4. **Handles optionals gracefully** - Sensible defaults for missing protobuf fields
5. **Maintains type safety** - Strong typing throughout the conversion process

## Module Structure

The `conversion.rs` module is organized into sections:

```
conversion.rs
├── AlkaneId Conversions
│   ├── convert_alkane_id()
│   └── to_proto_alkane_id()
├── U128 Conversions
│   ├── convert_u128()
│   ├── to_proto_u128()
│   └── convert_u128_opt()
├── AlkaneTransfer Conversions
│   ├── convert_alkane_transfer()
│   ├── to_proto_alkane_transfer()
│   ├── convert_alkane_transfers()
│   └── to_proto_alkane_transfers()
├── Context Conversions
│   ├── convert_context()
│   └── to_proto_context()
├── Trace Context Conversions (std-only)
│   ├── convert_enter_context()
│   ├── to_proto_trace_context()
│   ├── convert_extended_call_response()
│   ├── to_proto_extended_call_response()
│   └── convert_exit_context()
├── Trace Event Conversions (std-only)
│   ├── convert_trace_event()
│   └── convert_trace()
└── Helper Functions
    ├── extract_alkane_id_or_default()
    ├── extract_u128_or_zero()
    ├── convert_alkane_ids()
    └── to_proto_alkane_ids()
```

## Naming Conventions

- `convert_*`: Convert from protobuf to domain type
- `to_proto_*`: Convert from domain type to protobuf
- `extract_*`: Helper functions that handle `Option<T>` with defaults

## Usage Examples

### Basic Conversion

```rust
use alkanes_cli_common::conversion::{convert_alkane_id, to_proto_alkane_id};
use alkanes_support::proto::alkanes as alkanes_pb;

// From protobuf to domain
let proto_id = alkanes_pb::AlkaneId {
    block: Some(alkanes_pb::Uint128 { lo: 100, hi: 0 }),
    tx: Some(alkanes_pb::Uint128 { lo: 5, hi: 0 }),
};
let domain_id = convert_alkane_id(proto_id);

// From domain to protobuf
let back_to_proto = to_proto_alkane_id(domain_id);
```

### Batch Conversions

```rust
use alkanes_cli_common::conversion::convert_alkane_transfers;

// Convert multiple transfers at once
let proto_transfers = vec![/* ... */];
let parcel = convert_alkane_transfers(proto_transfers);
```

### Helper Functions

```rust
use alkanes_cli_common::conversion::extract_alkane_id_or_default;

// Safely extract with default fallback
let id = extract_alkane_id_or_default(optional_proto_id);
```

## Design Principles

1. **Pure Functions**: All conversions are stateless and pure
2. **Graceful Defaults**: Missing optional fields use sensible defaults (0 for numbers)
3. **No Panics**: All conversions handle edge cases without panicking
4. **Type Safety**: Strong typing prevents accidental misuse
5. **Testability**: Each conversion has unit tests

## Integration with Existing Code

The conversion module complements, but does not replace, existing `From` trait implementations in modules like `trace.rs`. Those implementations are specific to display formatting and should remain.

The conversion module should be used when:
- Converting protobuf types in new code
- Avoiding orphan rule violations
- Needing bidirectional conversions
- Working with `alkanes-support` types in a general context

Existing `From` implementations should be kept when:
- They're specific to a module's domain (e.g., trace display)
- They provide semantic meaning beyond simple conversion
- They're already working well and don't need changes

## Testing

The conversion module includes comprehensive unit tests covering:
- Roundtrip conversions (proto -> domain -> proto)
- Edge cases (u128::MAX, zero values)
- Missing optional fields
- Helper function behavior

Run tests with:
```bash
cargo test --package alkanes-cli-common --lib conversion::tests
```

## Future Enhancements

Potential areas for expansion:
1. Add conversions for more protobuf types as needed
2. Implement custom error types instead of panicking
3. Add benchmarks for performance-critical conversions
4. Consider macro-based conversion generation for boilerplate reduction

## References

- Inspired by `subfrost-common/src/conversion.rs`
- Rust orphan rule: [E0117](https://doc.rust-lang.org/error-index.html#E0117)
- Protobuf definitions: `crates/alkanes-support/proto/alkanes.proto`
