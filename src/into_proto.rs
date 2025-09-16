// Copyright 2024-present, Fractal Industries, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # IntoProto Trait
//!
//! This module defines the `IntoProto` trait, which provides a uniform way
//! to convert native Rust types into their Protobuf-generated counterparts.
//! This trait is a solution to the orphan rule, which prevents implementing
//! foreign traits for foreign types. By defining our own `IntoProto` trait
//! locally, we can implement it for any type, providing a clean and
//! consistent conversion API across the crate.

use crate::WasmHost;
use alkanes_proto::alkanes;
use alkanes_support::view::{Balance, Outpoint, Wallet};
use bitcoin::{hashes::Hash, OutPoint, TxOut};
use protobuf::{Message, MessageField};
use protorune_support::balance_sheet::{BalanceSheet};
use alkanes_support::trace::{TraceEvent, TraceContext, TraceResponse};

pub trait IntoProto<T> {
    fn into_proto(self) -> T;
}

impl IntoProto<alkanes_proto::alkanes::Outpoint> for OutPoint {
    fn into_proto(self) -> alkanes_proto::alkanes::Outpoint {
        let mut output = alkanes_proto::alkanes::Outpoint::new();
        output.txid = self.txid.to_byte_array().to_vec();
        output.vout = self.vout;
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::Output> for TxOut {
    fn into_proto(self) -> alkanes_proto::alkanes::Output {
        let mut output = alkanes_proto::alkanes::Output::new();
        output.value = self.value.to_sat();
        output.script = self.script_pubkey.to_bytes();
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::OutpointResponse> for Outpoint {
    fn into_proto(self) -> alkanes_proto::alkanes::OutpointResponse {
        let mut output = alkanes_proto::alkanes::OutpointResponse::new();
        output.outpoint = MessageField::some(self.outpoint.into_proto());
        output.output = MessageField::some(self.output.into_proto());
        output.height = self.height;
        output.txindex = self.txindex;
        let balances = self
            .balances
            .into_iter()
            .map(|balance| balance.into_proto())
            .collect();
        let mut balances_proto = alkanes_proto::alkanes::Balances::new();
        balances_proto.entries = balances;
        output.balances = balances_proto.write_to_bytes().unwrap();
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::WalletResponse> for Wallet {
    fn into_proto(self) -> alkanes_proto::alkanes::WalletResponse {
        let mut output = alkanes_proto::alkanes::WalletResponse::new();
        output.outpoints = self
            .outpoints
            .into_iter()
            .map(|outpoint| outpoint.into_proto())
            .collect();
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::Balance> for Balance {
    fn into_proto(self) -> alkanes_proto::alkanes::Balance {
        let mut output = alkanes_proto::alkanes::Balance::new();
        let packed_rune_id =
            (u128::from(self.rune_id.height.into_option().unwrap_or_default())) << 32
                | (u128::from(self.rune_id.txindex.into_option().unwrap_or_default()));
        output.rune_id = MessageField::some(alkanes_proto::alkanes::Uint128 {
            hi: (packed_rune_id >> 64) as u64,
            lo: packed_rune_id as u64,
            ..Default::default()
        });
        output.amount = MessageField::some(alkanes_proto::alkanes::Uint128 {
            hi: (self.amount >> 64) as u64,
            lo: self.amount as u64,
            ..Default::default()
        });
        output
    }
}

impl IntoProto<Vec<alkanes_proto::alkanes::Rune>> for Vec<alkanes_support::view::Rune> {
    fn into_proto(self) -> Vec<alkanes_proto::alkanes::Rune> {
        self.into_iter()
            .map(|rune| {
                let mut output = alkanes_proto::alkanes::Rune::new();
                output.runeId = MessageField::some(alkanes_proto::alkanes::ProtoruneRuneId {
                    height: MessageField::some(alkanes_proto::alkanes::Uint128 {
                        hi: (u128::from(rune.rune_id.height.clone().into_option().unwrap_or_default()) >> 64)
                            as u64,
                        lo: u128::from(rune.rune_id.height.into_option().unwrap_or_default()) as u64,
                        ..Default::default()
                    }),
                    txindex: MessageField::some(alkanes_proto::alkanes::Uint128 {
                        hi: (u128::from(rune.rune_id.txindex.clone().into_option().unwrap_or_default()) >> 64)
                            as u64,
                        lo: u128::from(rune.rune_id.txindex.into_option().unwrap_or_default()) as u64,
                        ..Default::default()
                    }),
                    ..Default::default()
                });
                output.name = rune.name;
                output.symbol = rune.symbol;
                output.spacers = rune.spacers;
                output.divisibility = rune.divisibility;
                output
            })
            .collect()
    }
}


impl IntoProto<alkanes_proto::alkanes::Balances> for BalanceSheet<WasmHost> {
    fn into_proto(self) -> alkanes_proto::alkanes::Balances {
        let mut balances = alkanes_proto::alkanes::Balances::new();
        balances.entries = self
            .balances
            .into_iter()
            .map(|(rune_id, amount)| {
                let mut balance = alkanes_proto::alkanes::Balance::new();
                let packed_rune_id =
                    (u128::from(rune_id.height.into_option().unwrap_or_default())) << 32
                        | (u128::from(rune_id.txindex.into_option().unwrap_or_default()));
                balance.rune_id = MessageField::some(alkanes_proto::alkanes::Uint128 {
                    hi: (packed_rune_id >> 64) as u64,
                    lo: packed_rune_id as u64,
                    ..Default::default()
                });
                balance.amount = MessageField::some(alkanes_proto::alkanes::Uint128 {
                    hi: (amount >> 64) as u64,
                    lo: amount as u64,
                    ..Default::default()
                });
                balance
            })
            .collect();
        balances
    }
}



impl IntoProto<alkanes_proto::alkanes::AlkaneId> for alkanes_support::id::AlkaneId {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkaneId {
        let mut output = alkanes_proto::alkanes::AlkaneId::new();
        output.block = MessageField::some(alkanes_proto::alkanes::Uint128 {
            hi: (self.block >> 64) as u64,
            lo: self.block as u64,
            ..Default::default()
        });
        output.tx = MessageField::some(alkanes_proto::alkanes::Uint128 {
            hi: (self.tx >> 64) as u64,
            lo: self.tx as u64,
            ..Default::default()
        });
        output
    }
}


impl IntoProto<alkanes_proto::alkanes::AlkanesTrace> for alkanes_support::trace::Trace {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkanesTrace {
        let mut output = alkanes_proto::alkanes::AlkanesTrace::new();
        output.events = self
            .0.lock()
            .unwrap()
            .clone()
            .into_iter()
            .map(|event| event.into_proto())
            .collect();
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::AlkanesTraceEvent> for TraceEvent {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkanesTraceEvent {
        let mut result = alkanes_proto::alkanes::AlkanesTraceEvent::new();
        result.event = Some(match self {
            TraceEvent::EnterCall(v) => {
                let mut context: alkanes_proto::alkanes::AlkanesEnterContext = v.into_proto();
                context.call_type = protobuf::EnumOrUnknown::from_i32(1);
                alkanes_proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterStaticcall(v) => {
                let mut context: alkanes_proto::alkanes::AlkanesEnterContext = v.into_proto();
                context.call_type = protobuf::EnumOrUnknown::from_i32(3);
                alkanes_proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterDelegatecall(v) => {
                let mut context: alkanes_proto::alkanes::AlkanesEnterContext = v.into_proto();
                context.call_type = protobuf::EnumOrUnknown::from_i32(2);
                alkanes_proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::ReturnContext(v) => {
                let mut context: alkanes_proto::alkanes::AlkanesExitContext = v.into_proto();
                context.status = protobuf::EnumOrUnknown::from_i32(0);
                alkanes_proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::RevertContext(v) => {
                let mut context: alkanes_proto::alkanes::AlkanesExitContext = v.into_proto();
                context.status = protobuf::EnumOrUnknown::from_i32(1);
                alkanes_proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::CreateAlkane(v) => {
                let mut creation = alkanes_proto::alkanes::AlkanesCreate::new();
                creation.new_alkane = MessageField::some(v.into_proto());
                alkanes_proto::alkanes::alkanes_trace_event::Event::CreateAlkane(creation)
            }
        });
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkanesEnterContext {
        let mut result = alkanes_proto::alkanes::AlkanesEnterContext::new();
        result.context = MessageField::some(self.into_proto());
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::TraceContext> for TraceContext {
    fn into_proto(self) -> alkanes_proto::alkanes::TraceContext {
        let mut result = alkanes_proto::alkanes::TraceContext::new();
        result.inner = MessageField::some(self.inner.into_proto());
        result.fuel = self.fuel;
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::Context> for alkanes_support::context::Context {
    fn into_proto(self) -> alkanes_proto::alkanes::Context {
        let mut result = alkanes_proto::alkanes::Context::new();
        result.myself = MessageField::some(self.myself.into_proto());
        result.caller = MessageField::some(self.caller.into_proto());
        result.vout = self.vout as u32;
        result.incoming_alkanes = self
            .incoming_alkanes
            .0
            .into_iter()
            .map(|v| v.into_proto())
            .collect::<Vec<alkanes_proto::alkanes::AlkaneTransfer>>();
        result.inputs = self
            .inputs
            .into_iter()
            .map(|v| v.into_proto())
            .collect::<Vec<alkanes_proto::alkanes::Uint128>>();
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::Uint128> for u128 {
    fn into_proto(self) -> alkanes_proto::alkanes::Uint128 {
        let mut result = alkanes_proto::alkanes::Uint128::new();
        result.lo = (self & u64::MAX as u128) as u64;
        result.hi = (self >> 64) as u64;
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::AlkaneTransfer> for alkanes_support::parcel::AlkaneTransfer {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkaneTransfer {
        let mut result = alkanes_proto::alkanes::AlkaneTransfer::new();
        result.id = MessageField::some(self.id.into_proto());
        result.value = MessageField::some(self.value.into_proto());
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::AlkanesExitContext> for TraceResponse {
    fn into_proto(self) -> alkanes_proto::alkanes::AlkanesExitContext {
        let mut result = alkanes_proto::alkanes::AlkanesExitContext::new();
        result.response = MessageField::some(self.inner.into_proto());
        result
    }
}

impl IntoProto<alkanes_proto::alkanes::ExtendedCallResponse> for alkanes_support::response::ExtendedCallResponse {
    fn into_proto(self) -> alkanes_proto::alkanes::ExtendedCallResponse {
        let mut result = alkanes_proto::alkanes::ExtendedCallResponse::new();
        result.alkanes = self.alkanes.0.into_iter().map(|v| v.into_proto()).collect();
        result.storage = self.storage.0.into_iter().map(|(k, v)| {
            let mut kvp = alkanes_proto::alkanes::KeyValuePair::new();
            kvp.key = k;
            kvp.value = v;
            kvp
        }).collect();
        result.data = self.data;
        result
    }
}