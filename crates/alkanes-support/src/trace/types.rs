use crate::context::Context;
use crate::id::AlkaneId;
use crate::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use crate::proto;
use crate::response::ExtendedCallResponse;
use crate::utils::field_or_default;
use protobuf::{Message, MessageField};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    pub inner: Context,
    pub target: AlkaneId,
    pub fuel: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TraceResponse {
    pub inner: ExtendedCallResponse,
    pub fuel_used: u64,
}

#[derive(Debug, Clone)]
pub enum TraceEvent {
    EnterDelegatecall(TraceContext),
    EnterStaticcall(TraceContext),
    EnterCall(TraceContext),
    RevertContext(TraceResponse),
    ReturnContext(TraceResponse),
    CreateAlkane(AlkaneId),
}

impl Into<TraceResponse> for ExtendedCallResponse {
    fn into(self) -> TraceResponse {
        TraceResponse {
            inner: self,
            fuel_used: 0,
        }
    }
}

impl Into<TraceContext> for Context {
    fn into(self) -> TraceContext {
        let target = self.myself.clone();
        TraceContext {
            inner: self,
            target,
            fuel: 0,
        }
    }
}

impl Into<proto::alkanes::Context> for Context {
    fn into(self) -> proto::alkanes::Context {
        let mut result = proto::alkanes::Context::new();
        result.myself = MessageField::some(self.myself.into());
        result.caller = MessageField::some(self.caller.into());
        result.vout = self.vout as u32;
        result.incoming_alkanes = self
            .incoming_alkanes
            .0
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<proto::alkanes::AlkaneTransfer>>();
        result.inputs = self
            .inputs
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<proto::alkanes::Uint128>>();
        result
    }
}

impl Into<proto::alkanes::AlkanesExitContext> for TraceResponse {
    fn into(self) -> proto::alkanes::AlkanesExitContext {
        let mut result = proto::alkanes::AlkanesExitContext::new();
        result.response = MessageField::some(self.inner.into());
        result
    }
}

impl Into<proto::alkanes::TraceContext> for TraceContext {
    fn into(self) -> proto::alkanes::TraceContext {
        let mut result = proto::alkanes::TraceContext::new();
        result.inner = MessageField::some(self.inner.into());
        result.fuel = self.fuel;
        result
    }
}

impl Into<proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn into(self) -> proto::alkanes::AlkanesEnterContext {
        let mut result = proto::alkanes::AlkanesEnterContext::new();
        result.context = MessageField::some(self.into());
        result
    }
}

impl Into<proto::alkanes::AlkanesTraceEvent> for TraceEvent {
    fn into(self) -> proto::alkanes::AlkanesTraceEvent {
        let mut result = proto::alkanes::AlkanesTraceEvent::new();
        result.event = Some(match self {
            TraceEvent::EnterCall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = protobuf::EnumOrUnknown::from_i32(1);
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterStaticcall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = protobuf::EnumOrUnknown::from_i32(3);
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterDelegatecall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = protobuf::EnumOrUnknown::from_i32(2);
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::ReturnContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = protobuf::EnumOrUnknown::from_i32(0);
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::RevertContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = protobuf::EnumOrUnknown::from_i32(1);
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::CreateAlkane(v) => {
                let mut creation = proto::alkanes::AlkanesCreate::new();
                creation.new_alkane = MessageField::some(v.into());
                proto::alkanes::alkanes_trace_event::Event::CreateAlkane(creation)
            }
        });
        result
    }
}

impl Into<TraceResponse> for proto::alkanes::ExtendedCallResponse {
    fn into(self) -> TraceResponse {
        <proto::alkanes::ExtendedCallResponse as Into<ExtendedCallResponse>>::into(self).into()
    }
}

impl From<TraceResponse> for ExtendedCallResponse {
    fn from(v: TraceResponse) -> ExtendedCallResponse {
        v.inner.into()
    }
}

impl From<TraceContext> for Context {
    fn from(v: TraceContext) -> Context {
        v.inner
    }
}

impl From<proto::alkanes::Context> for Context {
    fn from(v: proto::alkanes::Context) -> Context {
        Context {
            myself: v
                .myself
                .into_option()
                .ok_or("")
                .and_then(|v| Ok(v.into()))
                .unwrap_or_else(|_| AlkaneId::default()),
            caller: v
                .caller
                .into_option()
                .ok_or("")
                .and_then(|v| Ok(v.into()))
                .unwrap_or_else(|_| AlkaneId::default()),
            vout: v.vout,
            incoming_alkanes: AlkaneTransferParcel(
                v.incoming_alkanes
                    .into_iter()
                    .map(|v| v.into())
                    .collect::<Vec<AlkaneTransfer>>(),
            ),
            inputs: v
                .inputs
                .into_iter()
                .map(|input| input.into())
                .collect::<Vec<u128>>(),
        }
    }
}

impl From<proto::alkanes::AlkanesExitContext> for TraceResponse {
    fn from(v: proto::alkanes::AlkanesExitContext) -> TraceResponse {
        let response = v
            .response
            .into_option()
            .ok_or("")
            .and_then(|v| Ok(v.into()))
            .unwrap_or_else(|_| ExtendedCallResponse::default());
        TraceResponse {
            inner: response,
            fuel_used: 0,
        }
    }
}

impl From<proto::alkanes::TraceContext> for TraceContext {
    fn from(v: proto::alkanes::TraceContext) -> Self {
        Self {
            inner: v
                .inner
                .into_option()
                .ok_or("")
                .and_then(|v| Ok(v.into()))
                .unwrap_or_else(|_| Context::default()),
            fuel: v.fuel.into(),
            target: AlkaneId::default(),
        }
    }
}

impl From<proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn from(v: proto::alkanes::AlkanesEnterContext) -> TraceContext {
        let mut context: TraceContext = field_or_default(v.context); //.into_option().ok_or("").and_then(|v| Ok(v.into())).unwrap_or_else(|_| TraceContext::default());
        context.target = match v.call_type.value() {
            1 => context.inner.myself.clone(),
            3 => context.inner.myself.clone(),
            2 => context.inner.caller.clone(),
            _ => context.inner.myself.clone(),
        };
        context
    }
}

impl From<proto::alkanes::AlkanesTraceEvent> for TraceEvent {
    fn from(v: proto::alkanes::AlkanesTraceEvent) -> Self {
        if v.event.is_some() {
            match v.event.unwrap() {
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context) => {
                    match context.call_type.value() {
                        1 => TraceEvent::EnterCall(context.into()),
                        2 => TraceEvent::EnterDelegatecall(context.into()),
                        3 => TraceEvent::EnterStaticcall(context.into()),
                        _ => TraceEvent::EnterCall(context.into()),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::ExitContext(v) => {
                    match v.status.value() {
                        0 => TraceEvent::ReturnContext(field_or_default(v.response)),
                        1 => TraceEvent::RevertContext(field_or_default(v.response)),
                        _ => TraceEvent::RevertContext(field_or_default(v.response)),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::CreateAlkane(v) => {
                    TraceEvent::CreateAlkane(field_or_default(v.new_alkane))
                }
            }
        } else {
            TraceEvent::CreateAlkane(AlkaneId { block: 0, tx: 0 })
        }
    }
}

#[derive(Debug, Default)]
pub struct Trace(pub Arc<Mutex<Vec<TraceEvent>>>);

impl Trace {
    pub fn clock(&self, event: TraceEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Clone for Trace {
    fn clone(&self) -> Self {
        Trace(self.0.clone())
    }
}

impl Into<proto::alkanes::AlkanesTrace> for Vec<TraceEvent> {
    fn into(self) -> proto::alkanes::AlkanesTrace {
        let mut result = proto::alkanes::AlkanesTrace::new();
        result.events = self
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<proto::alkanes::AlkanesTraceEvent>>();
        result
    }
}

impl Into<proto::alkanes::AlkanesTrace> for Trace {
    fn into(self) -> proto::alkanes::AlkanesTrace {
        self.0.lock().unwrap().clone().into()
    }
}

impl Into<Vec<TraceEvent>> for proto::alkanes::AlkanesTrace {
    fn into(self) -> Vec<TraceEvent> {
        self.events
            .into_iter()
            .map(|v| v.into())
            .collect::<Vec<TraceEvent>>()
    }
}

impl Into<Trace> for proto::alkanes::AlkanesTrace {
    fn into(self) -> Trace {
        Trace(Arc::new(Mutex::new(self.into())))
    }
}

impl TryFrom<Vec<u8>> for Trace {
    type Error = anyhow::Error;
    fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
        let reference: &[u8] = v.as_ref();
        Ok(proto::alkanes::AlkanesTrace::parse_from_bytes(reference)?.into())
    }
}
