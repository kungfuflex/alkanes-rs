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
                caller = ctx.caller.unwrap_or_default().into();
                contract_id = Some(ctx.myself.unwrap_or_default().into());
                
                // Extract input data
                input_data = ctx.inputs.iter().flat_map(|u| {
                    let val: u128 = (u.hi as u128) << 64 | u.lo as u128;
                    val.to_le_bytes().to_vec()
                }).collect();

                // Extract value from the first incoming alkane transfer
                if let Some(transfer) = ctx.incoming_alkanes.first() {
                    value = transfer.value.clone().map(|v| v.into());
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
            context: e.context.unwrap().into(),
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
            new_alkane: e.new_alkane.unwrap().into(),
        }
    }
}

impl From<alkanes_support::proto::alkanes::TraceContext> for TraceContext {
    fn from(t: alkanes_support::proto::alkanes::TraceContext) -> Self {
        Self {
            inner: t.inner.unwrap().into(),
            fuel: t.fuel,
        }
    }
}

impl From<alkanes_support::proto::alkanes::Context> for Context {
    fn from(c: alkanes_support::proto::alkanes::Context) -> Self {
        Self {
            myself: c.myself.unwrap().into(),
            caller: c.caller.unwrap().into(),
            inputs: c.inputs.into_iter().map(Into::into).collect(),
            vout: c.vout,
            incoming_alkanes: c.incoming_alkanes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<alkanes_support::proto::alkanes::AlkaneTransfer> for AlkaneTransfer {
    fn from(t: alkanes_support::proto::alkanes::AlkaneTransfer) -> Self {
        Self {
            id: t.id.unwrap().into(),
            value: t.value.unwrap().into(),
        }
    }
}


impl From<alkanes_support::proto::alkanes::AlkaneId> for ContractId {
    fn from(id: alkanes_support::proto::alkanes::AlkaneId) -> Self {
        Self {
            block: id.block.map(Into::into),
            tx: id.tx.map(Into::into),
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

/// Format a trace for pretty printing with colorful emojis and YAML-like tree structure
/// This function works with alkanes_support::trace::Trace
pub fn format_trace_pretty(trace: &alkanes_support::trace::Trace) -> String {
    let events = trace.0.lock().unwrap();
    let mut output = String::new();
    
    // Header with colorful styling
    output.push_str("🔍 ═══════════════════════════════════════════════════════════════\n");
    output.push_str("🧪                    ALKANES EXECUTION TRACE                    🧪\n");
    output.push_str("🔍 ═══════════════════════════════════════════════════════════════\n\n");
    
    if events.is_empty() {
        output.push_str("📭 trace:\n");
        output.push_str("    events: []\n");
        output.push_str("    status: ✅ parsed_successfully\n");
        output.push_str("    note: \"No execution events found\"\n");
    } else {
        output.push_str("📊 trace:\n");
        output.push_str(&format!("    total_events: {}\n", events.len()));
        output.push_str("    events:\n");
        
        for (i, event) in events.iter().enumerate() {
            let is_last = i == events.len() - 1;
            let tree_prefix = if is_last { "    └─" } else { "    ├─" };
            let indent_prefix = if is_last { "      " } else { "    │ " };
            
            match event {
                alkanes_support::trace::TraceEvent::CreateAlkane(id) => {
                    output.push_str(&format!("{} 🏗️  create_alkane:\n", tree_prefix));
                    output.push_str(&format!("{}    alkane_id:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, id.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, id.tx));
                    output.push_str(&format!("{}    status: ✅ created\n", indent_prefix));
                },
                alkanes_support::trace::TraceEvent::EnterCall(ctx) => {
                    output.push_str(&format!("{} 📞 call:\n", tree_prefix));
                    output.push_str(&format!("{}    target:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.target.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.target.tx));
                    output.push_str(&format!("{}    caller:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.inner.caller.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.inner.caller.tx));
                    output.push_str(&format!("{}    ⛽ fuel_allocated: {}\n", indent_prefix, ctx.fuel));
                    
                    if !ctx.inner.inputs.is_empty() {
                        output.push_str(&format!("{}    📥 inputs:\n", indent_prefix));
                        for (j, input) in ctx.inner.inputs.iter().enumerate() {
                            let input_tree = if j == ctx.inner.inputs.len() - 1 { "└─" } else { "├─" };
                            output.push_str(&format!("{}      {} [{}]: {}\n", indent_prefix, input_tree, j, input));
                        }
                    } else {
                        output.push_str(&format!("{}    📥 inputs: []\n", indent_prefix));
                    }
                },
                alkanes_support::trace::TraceEvent::EnterDelegatecall(ctx) => {
                    output.push_str(&format!("{} 🔄 delegatecall:\n", tree_prefix));
                    output.push_str(&format!("{}    target:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.target.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.target.tx));
                    output.push_str(&format!("{}    caller:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.inner.caller.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.inner.caller.tx));
                    output.push_str(&format!("{}    ⛽ fuel_allocated: {}\n", indent_prefix, ctx.fuel));
                },
                alkanes_support::trace::TraceEvent::EnterStaticcall(ctx) => {
                    output.push_str(&format!("{} 🔒 staticcall:\n", tree_prefix));
                    output.push_str(&format!("{}    target:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.target.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.target.tx));
                    output.push_str(&format!("{}    caller:\n", indent_prefix));
                    output.push_str(&format!("{}      block: {}\n", indent_prefix, ctx.inner.caller.block));
                    output.push_str(&format!("{}      tx: {}\n", indent_prefix, ctx.inner.caller.tx));
                    output.push_str(&format!("{}    ⛽ fuel_allocated: {}\n", indent_prefix, ctx.fuel));
                },
                alkanes_support::trace::TraceEvent::ReturnContext(resp) => {
                    output.push_str(&format!("{} ✅ return:\n", tree_prefix));
                    output.push_str(&format!("{}    ⛽ fuel_used: {}\n", indent_prefix, resp.fuel_used));
                    
                    if !resp.inner.data.is_empty() {
                        output.push_str(&format!("{}    📤 return_data:\n", indent_prefix));
                        output.push_str(&format!("{}      hex: \"{}\"\n", indent_prefix, hex::encode(&resp.inner.data)));
                        output.push_str(&format!("{}      length: {} bytes\n", indent_prefix, resp.inner.data.len()));
                    } else {
                        output.push_str(&format!("{}    📤 return_data: null\n", indent_prefix));
                    }
                    
                    if !resp.inner.alkanes.0.is_empty() {
                        output.push_str(&format!("{}    🪙 alkane_transfers:\n", indent_prefix));
                        for (j, transfer) in resp.inner.alkanes.0.iter().enumerate() {
                            let transfer_tree = if j == resp.inner.alkanes.0.len() - 1 { "└─" } else { "├─" };
                            output.push_str(&format!("{}      {} transfer_{}:\n", indent_prefix, transfer_tree, j));
                            output.push_str(&format!("{}      {}   alkane_id:\n", indent_prefix, if j == resp.inner.alkanes.0.len() - 1 { " " } else { "│" }));
                            output.push_str(&format!("{}      {}     block: {}\n", indent_prefix, if j == resp.inner.alkanes.0.len() - 1 { " " } else { "│" }, transfer.id.block));
                            output.push_str(&format!("{}      {}     tx: {}\n", indent_prefix, if j == resp.inner.alkanes.0.len() - 1 { " " } else { "│" }, transfer.id.tx));
                            output.push_str(&format!("{}      {}   amount: {}\n", indent_prefix, if j == resp.inner.alkanes.0.len() - 1 { " " } else { "│" }, transfer.value));
                        }
                    } else {
                        output.push_str(&format!("{}    🪙 alkane_transfers: []\n", indent_prefix));
                    }
                },
                alkanes_support::trace::TraceEvent::RevertContext(resp) => {
                    output.push_str(&format!("{} ❌ revert:\n", tree_prefix));
                    output.push_str(&format!("{}    ⛽ fuel_used: {}\n", indent_prefix, resp.fuel_used));
                    
                    if !resp.inner.data.is_empty() {
                        output.push_str(&format!("{}    🚨 error_data:\n", indent_prefix));
                        output.push_str(&format!("{}      hex: \"{}\"\n", indent_prefix, hex::encode(&resp.inner.data)));
                        output.push_str(&format!("{}      length: {} bytes\n", indent_prefix, resp.inner.data.len()));
                    } else {
                        output.push_str(&format!("{}    🚨 error_data: null\n", indent_prefix));
                    }
                },
            }
            
            // Add spacing between events except for the last one
            if !is_last {
                output.push_str("    │\n");
            }
        }
    }
    
    output.push_str("\n🎯 ═══════════════════════════════════════════════════════════════\n");
    output.push_str("✨                      TRACE COMPLETE                         ✨\n");
    output.push_str("🎯 ═══════════════════════════════════════════════════════════════\n");
    output
}

/// Convert a trace to JSON format for raw output
pub fn trace_to_json(trace: &alkanes_support::trace::Trace) -> JsonValue {
    use serde_json::json;
    
    let events = trace.0.lock().unwrap();
    let mut json_events = Vec::new();
    
    for event in events.iter() {
        let json_event = match event {
            alkanes_support::trace::TraceEvent::CreateAlkane(id) => {
                json!({
                    "type": "create_alkane",
                    "alkane_id": {
                        "block": id.block,
                        "tx": id.tx
                    }
                })
            },
            alkanes_support::trace::TraceEvent::EnterCall(ctx) => {
                json!({
                    "type": "call",
                    "target": {
                        "block": ctx.target.block,
                        "tx": ctx.target.tx
                    },
                    "caller": {
                        "block": ctx.inner.caller.block,
                        "tx": ctx.inner.caller.tx
                    },
                    "fuel_allocated": ctx.fuel,
                    "inputs": ctx.inner.inputs
                })
            },
            alkanes_support::trace::TraceEvent::EnterDelegatecall(ctx) => {
                json!({
                    "type": "delegatecall",
                    "target": {
                        "block": ctx.target.block,
                        "tx": ctx.target.tx
                    },
                    "caller": {
                        "block": ctx.inner.caller.block,
                        "tx": ctx.inner.caller.tx
                    },
                    "fuel_allocated": ctx.fuel
                })
            },
            alkanes_support::trace::TraceEvent::EnterStaticcall(ctx) => {
                json!({
                    "type": "staticcall",
                    "target": {
                        "block": ctx.target.block,
                        "tx": ctx.target.tx
                    },
                    "caller": {
                        "block": ctx.inner.caller.block,
                        "tx": ctx.inner.caller.tx
                    },
                    "fuel_allocated": ctx.fuel
                })
            },
            alkanes_support::trace::TraceEvent::ReturnContext(resp) => {
                let alkane_transfers: Vec<JsonValue> = resp.inner.alkanes.0.iter().map(|transfer| {
                    json!({
                        "alkane_id": {
                            "block": transfer.id.block,
                            "tx": transfer.id.tx
                        },
                        "amount": transfer.value
                    })
                }).collect();
                
                json!({
                    "type": "return",
                    "fuel_used": resp.fuel_used,
                    "return_data": if resp.inner.data.is_empty() { json!(null) } else { json!(hex::encode(&resp.inner.data)) },
                    "alkane_transfers": alkane_transfers
                })
            },
            alkanes_support::trace::TraceEvent::RevertContext(resp) => {
                json!({
                    "type": "revert",
                    "fuel_used": resp.fuel_used,
                    "error_data": if resp.inner.data.is_empty() { json!(null) } else { json!(hex::encode(&resp.inner.data)) }
                })
            },
        };
        json_events.push(json_event);
    }
    
    json!({
        "trace": json_events
    })
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