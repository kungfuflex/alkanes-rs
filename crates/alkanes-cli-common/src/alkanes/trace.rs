// This file is part of the deezel project.
// Copyright (c) 2023, Casey Rodarmor, all rights reserved.
// Copyright (c) 2024, The Deezel Developers, all rights reserved.
// Deezel is licensed under the MIT license.
// See LICENSE file in the project root for full license information.

//! This module defines the structures for representing and displaying alkanes transaction traces.
//! It provides a native Rust representation of the trace data returned by the indexer,
//! along with implementations for serialization, deserialization, and pretty-printing.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::{String, ToString}, format};
#[cfg(feature = "std")]
use std::vec::Vec;
use core::fmt;

/// Represents a complete execution trace of a transaction, containing multiple calls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trace {
    #[serde(rename = "trace")]
    pub calls: Vec<Call>,
}

/// Represents a single call within a transaction trace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Call {
    pub caller: ContractId,
    #[serde(rename = "id")]
    pub contract_id: Option<ContractId>,
    #[serde(rename = "inputData", with = "hex_serde")]
    pub input_data: Vec<u8>,
    #[serde(rename = "value")]
    pub value: Option<U128>,
    pub events: Vec<Event>,
}

/// Represents a contract identifier (block and transaction index).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ContractId {
    pub block: Option<U128>,
    pub tx: Option<U128>,
}

/// Represents an event emitted during a call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Event {
    Enter(EnterContext),
    Exit(ExitContext),
    Create(Create),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnterContext {
    pub call_type: String,
    pub context: TraceContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitContext {
    pub status: String,
    // Omitting response for now as it's complex and not immediately needed for display
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Create {
    pub new_alkane: ContractId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceContext {
    pub inner: Context,
    pub fuel: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub myself: ContractId,
    pub caller: ContractId,
    pub inputs: Vec<U128>,
    pub vout: u32,
    pub incoming_alkanes: Vec<AlkaneTransfer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlkaneTransfer {
    pub id: ContractId,
    pub value: U128,
}

/// Represents a 64-bit unsigned integer, used for block and tx numbers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct U64 {
    pub lo: u64,
}

/// Represents a 128-bit unsigned integer, used for token values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct U128 {
    pub lo: u64,
    pub hi: u64,
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, call) in self.calls.iter().enumerate() {
            writeln!(f, "Call {i}:")?;
            if let (Some(b), Some(t)) = (call.caller.block.as_ref(), call.caller.tx.as_ref()) {
                writeln!(f, "  Caller: {}.{}", b.lo, t.lo)?;
            }
            if let Some(id) = &call.contract_id {
                 if let (Some(b), Some(t)) = (id.block.as_ref(), id.tx.as_ref()) {
                    writeln!(f, "  Contract: {}.{}", b.lo, t.lo)?;
                }
            }
            writeln!(f, "  Input Data: {}", hex::encode(&call.input_data))?;
            if let Some(value) = &call.value {
                 writeln!(f, "  Value: hi: {}, lo: {}", value.hi, value.lo)?;
            }
            writeln!(f, "  Events:")?;
            for (j, event) in call.events.iter().enumerate() {
                writeln!(f, "    Event {j}: {event:?}")?;
            }
        }
        Ok(())
    }
}

impl From<alkanes_support::proto::alkanes::Trace> for Trace {
    fn from(trace: alkanes_support::proto::alkanes::Trace) -> Self {
        let calls = trace.trace.iter().map(|c| c.clone().into()).collect();
        Self { calls }
    }
}

impl From<alkanes_support::proto::alkanes::AlkanesTrace> for Call {
    fn from(trace: alkanes_support::proto::alkanes::AlkanesTrace) -> Self {
        let mut caller = ContractId::default();
        let mut contract_id = None;
        let mut input_data = Vec::new();
        let mut value = None;

        // The first event is expected to be EnterContext, which contains the call details
        if let Some(first_event) = trace.events.first() {
            if let Some(alkanes_support::proto::alkanes::alkanes_trace_event::Event::EnterContext(enter_context)) = &first_event.event {
                let trace_ctx = enter_context.context.as_ref().cloned().unwrap_or_default();
                let ctx = trace_ctx.inner.as_ref().cloned().unwrap_or_default();
                caller = ctx.caller.clone().unwrap_or_default().into();
                contract_id = Some(ctx.myself.clone().unwrap_or_default().into());
                
                // Extract input data
                input_data = ctx.inputs.iter().flat_map(|u| {
                    let val: u128 = (u.hi as u128) << 64 | u.lo as u128;
                    val.to_le_bytes().to_vec()
                }).collect();

                // Extract value from the first incoming alkane transfer
                if let Some(transfer) = ctx.incoming_alkanes.first() {
                    value = transfer.value.clone().clone().map(|v| v.into());
                }
            }
        }


        Self {
            caller,
            contract_id,
            input_data,
            value,
            events: trace.events.into_iter().filter_map(|e| e.event).map(Into::into).collect(),
        }
    }
}

#[allow(unreachable_patterns)]
impl From<alkanes_support::proto::alkanes::alkanes_trace_event::Event> for Event {
    fn from(event: alkanes_support::proto::alkanes::alkanes_trace_event::Event) -> Self {
        match event {
            alkanes_support::proto::alkanes::alkanes_trace_event::Event::EnterContext(e) => Event::Enter(e.into()),
            alkanes_support::proto::alkanes::alkanes_trace_event::Event::ExitContext(e) => Event::Exit(e.into()),
            alkanes_support::proto::alkanes::alkanes_trace_event::Event::CreateAlkane(e) => Event::Create(e.into()),
            _ => Event::Unknown,
        }
    }
}

impl From<alkanes_support::proto::alkanes::AlkanesEnterContext> for EnterContext {
    fn from(e: alkanes_support::proto::alkanes::AlkanesEnterContext) -> Self {
        Self {
            call_type: format!("{:?}", e.call_type),
            context: e.context.clone().unwrap().into(),
        }
    }
}

impl From<alkanes_support::proto::alkanes::AlkanesExitContext> for ExitContext {
    fn from(e: alkanes_support::proto::alkanes::AlkanesExitContext) -> Self {
        Self {
            status: format!("{:?}", e.status),
        }
    }
}

impl From<alkanes_support::proto::alkanes::AlkanesCreate> for Create {
    fn from(e: alkanes_support::proto::alkanes::AlkanesCreate) -> Self {
        Self {
            new_alkane: e.new_alkane.clone().unwrap().into(),
        }
    }
}

impl From<alkanes_support::proto::alkanes::TraceContext> for TraceContext {
    fn from(t: alkanes_support::proto::alkanes::TraceContext) -> Self {
        Self {
            inner: t.inner.clone().unwrap().into(),
            fuel: t.fuel,
        }
    }
}

impl From<alkanes_support::proto::alkanes::Context> for Context {
    fn from(c: alkanes_support::proto::alkanes::Context) -> Self {
        Self {
            myself: c.myself.clone().unwrap().into(),
            caller: c.caller.clone().unwrap().into(),
            inputs: c.inputs.into_iter().map(Into::into).collect(),
            vout: c.vout,
            incoming_alkanes: c.incoming_alkanes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<alkanes_support::proto::alkanes::AlkaneTransfer> for AlkaneTransfer {
    fn from(t: alkanes_support::proto::alkanes::AlkaneTransfer) -> Self {
        Self {
            id: t.id.clone().unwrap().into(),
            value: t.value.clone().unwrap().into(),
        }
    }
}


impl From<alkanes_support::proto::alkanes::AlkaneId> for ContractId {
    fn from(id: alkanes_support::proto::alkanes::AlkaneId) -> Self {
        Self {
            block: id.block.clone().map(Into::into),
            tx: id.tx.clone().map(Into::into),
        }
    }
}

impl From<alkanes_support::proto::alkanes::Uint128> for U128 {
    fn from(u: alkanes_support::proto::alkanes::Uint128) -> Self {
        Self { lo: u.lo, hi: u.hi }
    }
}

/// Converts a Trace object to a raw JSON value.
pub fn to_raw_json(trace: &Trace) -> JsonValue {
    serde_json::to_value(trace).unwrap_or_else(|_| serde_json::json!({ "error": "Failed to serialize trace" }))
}

mod hex_serde {
    use serde::{Serializer, Deserializer, de::Error, Deserialize};
    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec::Vec};
    #[cfg(feature = "std")]
    use std::{string::String, vec::Vec};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(Error::custom)
    }
}