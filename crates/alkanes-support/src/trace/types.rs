use crate::context::Context;
use crate::id::AlkaneId;
use crate::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use crate::proto;
use crate::response::ExtendedCallResponse;
use prost::Message;
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
        let mut result = proto::alkanes::Context::default();
        result.myself = Some(self.myself.into());
        result.caller = Some(self.caller.into());
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
        let mut result = proto::alkanes::AlkanesExitContext::default();
        result.response = Some(self.inner.into());
        result
    }
}

impl Into<proto::alkanes::TraceContext> for TraceContext {
    fn into(self) -> proto::alkanes::TraceContext {
        let mut result = proto::alkanes::TraceContext::default();
        result.inner = Some(self.inner.into());
        result.fuel = self.fuel;
        result
    }
}

impl Into<proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn into(self) -> proto::alkanes::AlkanesEnterContext {
        let mut result = proto::alkanes::AlkanesEnterContext::default();
        result.context = Some(self.into());
        result
    }
}

impl Into<proto::alkanes::AlkanesTraceEvent> for TraceEvent {
    fn into(self) -> proto::alkanes::AlkanesTraceEvent {
        let mut result = proto::alkanes::AlkanesTraceEvent::default();
        result.event = Some(match self {
            TraceEvent::EnterCall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = proto::alkanes::AlkanesTraceCallType::Call.into();
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterStaticcall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = proto::alkanes::AlkanesTraceCallType::Staticcall.into();
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterDelegatecall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = proto::alkanes::AlkanesTraceCallType::Delegatecall.into();
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::ReturnContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = proto::alkanes::AlkanesTraceStatusFlag::Success.into();
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::RevertContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = proto::alkanes::AlkanesTraceStatusFlag::Failure.into();
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::CreateAlkane(v) => {
                let mut creation = proto::alkanes::AlkanesCreate::default();
                creation.new_alkane = Some(v.into());
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
                .ok_or("")
                .and_then(|v| Ok(v.into()))
                .unwrap_or_else(|_| AlkaneId::default()),
            caller: v
                .caller
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
        let mut context: TraceContext = v.context.clone().unwrap_or_default().into();
        context.target = match v.call_type() {
            proto::alkanes::AlkanesTraceCallType::Call => context.inner.myself.clone(),
            proto::alkanes::AlkanesTraceCallType::Staticcall => context.inner.myself.clone(),
            proto::alkanes::AlkanesTraceCallType::Delegatecall => context.inner.caller.clone(),
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
                    match context.call_type() {
                        proto::alkanes::AlkanesTraceCallType::Call => TraceEvent::EnterCall(context.into()),
                        proto::alkanes::AlkanesTraceCallType::Delegatecall => TraceEvent::EnterDelegatecall(context.into()),
                        proto::alkanes::AlkanesTraceCallType::Staticcall => TraceEvent::EnterStaticcall(context.into()),
                        _ => TraceEvent::EnterCall(context.into()),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::ExitContext(v) => {
                    match v.status() {
                        proto::alkanes::AlkanesTraceStatusFlag::Success => TraceEvent::ReturnContext(v.response.unwrap_or_default().into()),
                        proto::alkanes::AlkanesTraceStatusFlag::Failure => TraceEvent::RevertContext(v.response.unwrap_or_default().into()),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::CreateAlkane(v) => {
                    TraceEvent::CreateAlkane(v.new_alkane.unwrap_or_default().into())
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
        let mut result = proto::alkanes::AlkanesTrace::default();
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
        Ok(proto::alkanes::AlkanesTrace::decode(reference)?.into())
    }
}
