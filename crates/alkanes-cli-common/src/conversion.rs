//! Conversion utilities for protobuf types
//!
//! This module provides conversion functions between protobuf types from
//! `alkanes-support` and the domain types used throughout the application.
//!
//! This pattern is necessary to avoid violating Rust's orphan rule (E0117),
//! which prevents implementing a foreign trait (like `From`) for a foreign type.
//! By providing explicit conversion functions, we maintain clean separation
//! between external protobuf types and our internal domain models.
//!
//! # Design Pattern
//!
//! - Use `convert_*` functions for simple type conversions
//! - Use `from_proto_*` and `to_proto_*` for bidirectional conversions
//! - Keep conversions pure and stateless
//! - Handle optional fields gracefully with sensible defaults
//!
//! # Example
//!
//! ```rust,ignore
//! use alkanes_cli_common::conversion::{convert_alkane_id, to_proto_alkane_id};
//! use alkanes_support::proto::alkanes as alkanes_pb;
//! use alkanes_support::id::AlkaneId;
//!
//! // Convert from protobuf to domain type
//! let proto_id = alkanes_pb::AlkaneId {
//!     block: Some(alkanes_pb::Uint128 { lo: 100, hi: 0 }),
//!     tx: Some(alkanes_pb::Uint128 { lo: 5, hi: 0 }),
//! };
//! let domain_id = convert_alkane_id(proto_id);
//! assert_eq!(domain_id.block, 100);
//! assert_eq!(domain_id.tx, 5);
//!
//! // Convert back to protobuf
//! let back_to_proto = to_proto_alkane_id(domain_id);
//! ```

#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

use alkanes_support::proto::alkanes as alkanes_pb;
use alkanes_support::{
    context::Context,
    id::AlkaneId as AlkaneIdSupport,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::ExtendedCallResponse,
    storage::StorageMap,
};

#[cfg(feature = "std")]
use alkanes_support::trace::{Trace, TraceContext, TraceEvent, TraceResponse};

// ============================================================================
// AlkaneId Conversions
// ============================================================================

/// Convert protobuf AlkaneId to domain AlkaneId
///
/// Maps optional u128 values to u128, using the lo field.
/// Returns a default (0, 0) id if block or tx fields are missing.
pub fn convert_alkane_id(id: alkanes_pb::AlkaneId) -> AlkaneIdSupport {
    AlkaneIdSupport {
        block: id.block.map_or(0, |b| b.lo as u128),
        tx: id.tx.map_or(0, |t| t.lo as u128),
    }
}

/// Convert domain AlkaneId to protobuf AlkaneId
pub fn to_proto_alkane_id(id: AlkaneIdSupport) -> alkanes_pb::AlkaneId {
    alkanes_pb::AlkaneId {
        block: Some(alkanes_pb::Uint128 {
            lo: id.block as u64,
            hi: 0,
        }),
        tx: Some(alkanes_pb::Uint128 {
            lo: id.tx as u64,
            hi: 0,
        }),
    }
}

// ============================================================================
// U128 Conversions
// ============================================================================

/// Convert protobuf Uint128 to native u128
pub fn convert_u128(value: alkanes_pb::Uint128) -> u128 {
    ((value.hi as u128) << 64) | (value.lo as u128)
}

/// Convert native u128 to protobuf Uint128
pub fn to_proto_u128(value: u128) -> alkanes_pb::Uint128 {
    alkanes_pb::Uint128 {
        lo: value as u64,
        hi: (value >> 64) as u64,
    }
}

/// Convert optional protobuf Uint128 to native u128, with default of 0
pub fn convert_u128_opt(value: Option<alkanes_pb::Uint128>) -> u128 {
    value.map_or(0, convert_u128)
}

// ============================================================================
// AlkaneTransfer Conversions
// ============================================================================

/// Convert protobuf AlkaneTransfer to domain AlkaneTransfer
pub fn convert_alkane_transfer(transfer: alkanes_pb::AlkaneTransfer) -> AlkaneTransfer {
    AlkaneTransfer {
        id: transfer.id.map_or(Default::default(), convert_alkane_id),
        value: transfer.value.map_or(0, convert_u128),
    }
}

/// Convert domain AlkaneTransfer to protobuf AlkaneTransfer
pub fn to_proto_alkane_transfer(transfer: AlkaneTransfer) -> alkanes_pb::AlkaneTransfer {
    alkanes_pb::AlkaneTransfer {
        id: Some(to_proto_alkane_id(transfer.id)),
        value: Some(to_proto_u128(transfer.value)),
    }
}

/// Convert a vector of protobuf AlkaneTransfers to an AlkaneTransferParcel
pub fn convert_alkane_transfers(transfers: Vec<alkanes_pb::AlkaneTransfer>) -> AlkaneTransferParcel {
    AlkaneTransferParcel(
        transfers
            .into_iter()
            .map(convert_alkane_transfer)
            .collect(),
    )
}

/// Convert AlkaneTransferParcel to vector of protobuf AlkaneTransfers
pub fn to_proto_alkane_transfers(parcel: AlkaneTransferParcel) -> Vec<alkanes_pb::AlkaneTransfer> {
    parcel.0
        .into_iter()
        .map(to_proto_alkane_transfer)
        .collect()
}

// ============================================================================
// Context Conversions
// ============================================================================

/// Convert protobuf Context to domain Context
pub fn convert_context(ctx: alkanes_pb::Context) -> Context {
    Context {
        myself: ctx.myself.map_or(Default::default(), convert_alkane_id),
        caller: ctx.caller.map_or(Default::default(), convert_alkane_id),
        vout: ctx.vout,
        incoming_alkanes: convert_alkane_transfers(ctx.incoming_alkanes),
        inputs: ctx
            .inputs
            .into_iter()
            .map(convert_u128)
            .collect(),
    }
}

/// Convert domain Context to protobuf Context
pub fn to_proto_context(ctx: Context) -> alkanes_pb::Context {
    alkanes_pb::Context {
        myself: Some(to_proto_alkane_id(ctx.myself)),
        caller: Some(to_proto_alkane_id(ctx.caller)),
        vout: ctx.vout,
        incoming_alkanes: to_proto_alkane_transfers(ctx.incoming_alkanes),
        inputs: ctx
            .inputs
            .into_iter()
            .map(to_proto_u128)
            .collect(),
    }
}

// ============================================================================
// Trace Context Conversions (requires std feature for Arc/Mutex)
// ============================================================================

#[cfg(feature = "std")]
/// Convert protobuf AlkanesEnterContext to domain TraceContext
pub fn convert_enter_context(ctx: alkanes_pb::AlkanesEnterContext) -> TraceContext {
    let inner_ctx: Context = ctx
        .context
        .clone()
        .unwrap_or_default()
        .inner
        .map_or(Default::default(), convert_context);
    
    TraceContext {
        inner: inner_ctx,
        target: AlkaneIdSupport { block: 0, tx: 0 }, // Placeholder - may need to be extracted from context
        fuel: ctx.context.unwrap_or_default().fuel,
    }
}

#[cfg(feature = "std")]
/// Convert domain TraceContext to protobuf TraceContext
pub fn to_proto_trace_context(ctx: TraceContext) -> alkanes_pb::TraceContext {
    alkanes_pb::TraceContext {
        inner: Some(to_proto_context(ctx.inner)),
        fuel: ctx.fuel,
    }
}

// ============================================================================
// Response Conversions
// ============================================================================

#[cfg(feature = "std")]
/// Convert protobuf ExtendedCallResponse to domain ExtendedCallResponse
pub fn convert_extended_call_response(resp: alkanes_pb::ExtendedCallResponse) -> ExtendedCallResponse {
    ExtendedCallResponse {
        storage: StorageMap::from_iter(
            resp.storage
                .into_iter()
                .map(|kv| (kv.key, kv.value)),
        ),
        data: resp.data,
        alkanes: convert_alkane_transfers(resp.alkanes),
    }
}

#[cfg(feature = "std")]
/// Convert domain ExtendedCallResponse to protobuf ExtendedCallResponse
pub fn to_proto_extended_call_response(resp: ExtendedCallResponse) -> alkanes_pb::ExtendedCallResponse {
    alkanes_pb::ExtendedCallResponse {
        storage: resp
            .storage
            .0 // Access the inner HashMap of StorageMap
            .iter()
            .map(|(k, v)| alkanes_pb::KeyValuePair {
                key: k.clone(),
                value: v.clone(),
            })
            .collect(),
        data: resp.data,
        alkanes: to_proto_alkane_transfers(resp.alkanes),
    }
}

#[cfg(feature = "std")]
/// Convert protobuf AlkanesExitContext to domain TraceResponse
pub fn convert_exit_context(resp: alkanes_pb::AlkanesExitContext) -> TraceResponse {
    TraceResponse {
        inner: resp.response.map_or(Default::default(), convert_extended_call_response),
        fuel_used: 0, // Placeholder - fuel tracking may need enhancement
    }
}

// ============================================================================
// Trace Event Conversions
// ============================================================================

#[cfg(feature = "std")]
/// Convert protobuf AlkanesTraceEvent to domain TraceEvent
pub fn convert_trace_event(event: alkanes_pb::AlkanesTraceEvent) -> TraceEvent {
    match event.event {
        Some(alkanes_pb::alkanes_trace_event::Event::EnterContext(ctx)) => {
            TraceEvent::EnterCall(convert_enter_context(ctx))
        }
        Some(alkanes_pb::alkanes_trace_event::Event::ExitContext(ctx)) => {
            TraceEvent::ReturnContext(convert_exit_context(ctx))
        }
        Some(alkanes_pb::alkanes_trace_event::Event::CreateAlkane(c)) => {
            TraceEvent::CreateAlkane(c.new_alkane.map_or(Default::default(), convert_alkane_id))
        }
        None => panic!("unknown trace event"),
    }
}

#[cfg(feature = "std")]
/// Convert protobuf Trace to domain Trace
pub fn convert_trace(trace: alkanes_pb::Trace) -> Trace {
    let events = trace
        .trace
        .map_or(vec![], |t| {
            t.events
                .into_iter()
                .map(convert_trace_event)
                .collect()
        });
    Trace(Arc::new(Mutex::new(events)))
}

// ============================================================================
// Helper Functions for Common Patterns
// ============================================================================

/// Extract AlkaneId from optional protobuf AlkaneId, using default if None
pub fn extract_alkane_id_or_default(id: Option<alkanes_pb::AlkaneId>) -> AlkaneIdSupport {
    id.map_or(Default::default(), convert_alkane_id)
}

/// Extract u128 value from optional protobuf Uint128, using 0 if None
pub fn extract_u128_or_zero(value: Option<alkanes_pb::Uint128>) -> u128 {
    value.map_or(0, convert_u128)
}

/// Batch convert a vector of protobuf AlkaneIds to domain AlkaneIds
pub fn convert_alkane_ids(ids: Vec<alkanes_pb::AlkaneId>) -> Vec<AlkaneIdSupport> {
    ids.into_iter().map(convert_alkane_id).collect()
}

/// Batch convert a vector of domain AlkaneIds to protobuf AlkaneIds
pub fn to_proto_alkane_ids(ids: Vec<AlkaneIdSupport>) -> Vec<alkanes_pb::AlkaneId> {
    ids.into_iter().map(to_proto_alkane_id).collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alkane_id_roundtrip() {
        let original = AlkaneIdSupport { block: 123, tx: 456 };
        let proto = to_proto_alkane_id(original);
        let converted = convert_alkane_id(proto);
        assert_eq!(original.block, converted.block);
        assert_eq!(original.tx, converted.tx);
    }

    #[test]
    fn test_u128_roundtrip() {
        let values = vec![0u128, 1, 255, 65536, u64::MAX as u128, u128::MAX];
        for value in values {
            let proto = to_proto_u128(value);
            let converted = convert_u128(proto);
            assert_eq!(value, converted, "Failed for value: {}", value);
        }
    }

    #[test]
    fn test_alkane_transfer_conversion() {
        let proto = alkanes_pb::AlkaneTransfer {
            id: Some(alkanes_pb::AlkaneId {
                block: Some(alkanes_pb::Uint128 { lo: 10, hi: 0 }),
                tx: Some(alkanes_pb::Uint128 { lo: 20, hi: 0 }),
            }),
            value: Some(alkanes_pb::Uint128 { lo: 1000, hi: 0 }),
        };

        let transfer = convert_alkane_transfer(proto.clone());
        assert_eq!(transfer.id.block, 10);
        assert_eq!(transfer.id.tx, 20);
        assert_eq!(transfer.value, 1000);

        let back_to_proto = to_proto_alkane_transfer(transfer);
        assert_eq!(back_to_proto.id.unwrap().block.unwrap().lo, 10);
        assert_eq!(back_to_proto.value.unwrap().lo, 1000);
    }

    #[test]
    fn test_missing_fields_use_defaults() {
        let proto = alkanes_pb::AlkaneId {
            block: None,
            tx: None,
        };
        let converted = convert_alkane_id(proto);
        assert_eq!(converted.block, 0);
        assert_eq!(converted.tx, 0);
    }

    #[test]
    fn test_extract_helpers() {
        assert_eq!(extract_alkane_id_or_default(None).block, 0);
        assert_eq!(extract_u128_or_zero(None), 0);
        
        let id = alkanes_pb::AlkaneId {
            block: Some(alkanes_pb::Uint128 { lo: 5, hi: 0 }),
            tx: Some(alkanes_pb::Uint128 { lo: 10, hi: 0 }),
        };
        assert_eq!(extract_alkane_id_or_default(Some(id)).block, 5);
    }
}
