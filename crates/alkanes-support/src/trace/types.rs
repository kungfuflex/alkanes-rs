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
    ReceiveIntent {
        incoming_alkanes: AlkaneTransferParcel,
    },
    ValueTransfer {
        transfers: Vec<AlkaneTransfer>,
        redirect_to: u32,
    },
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
        proto::alkanes::Context {
            myself: Some(self.myself.into()),
            caller: Some(self.caller.into()),
            vout: self.vout as u32,
            incoming_alkanes: self
                .incoming_alkanes
                .0
                .into_iter()
                .map(|v| v.into())
                .collect::<Vec<proto::alkanes::AlkaneTransfer>>(),
            inputs: self
                .inputs
                .into_iter()
                .map(|v| v.into())
                .collect::<Vec<proto::alkanes::Uint128>>(),
        }
    }
}

impl Into<proto::alkanes::AlkanesExitContext> for TraceResponse {
    fn into(self) -> proto::alkanes::AlkanesExitContext {
        proto::alkanes::AlkanesExitContext {
            response: Some(self.inner.into()),
            ..Default::default()
        }
    }
}

impl Into<proto::alkanes::TraceContext> for TraceContext {
    fn into(self) -> proto::alkanes::TraceContext {
        proto::alkanes::TraceContext {
            inner: Some(self.inner.into()),
            fuel: self.fuel,
        }
    }
}

impl Into<proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn into(self) -> proto::alkanes::AlkanesEnterContext {
        proto::alkanes::AlkanesEnterContext {
            context: Some(self.into()),
            ..Default::default()
        }
    }
}

impl Into<proto::alkanes::AlkanesTraceEvent> for TraceEvent {
    fn into(self) -> proto::alkanes::AlkanesTraceEvent {
        let event = match self {
            TraceEvent::ReceiveIntent { incoming_alkanes } => {
                let receive_intent = proto::alkanes::AlkanesReceiveIntent {
                    incoming_alkanes: incoming_alkanes
                        .0
                        .into_iter()
                        .map(|v| v.into())
                        .collect::<Vec<proto::alkanes::AlkaneTransfer>>(),
                };
                proto::alkanes::alkanes_trace_event::Event::ReceiveIntent(receive_intent)
            }
            TraceEvent::ValueTransfer {
                transfers,
                redirect_to,
            } => {
                let value_transfer = proto::alkanes::AlkanesValueTransfer {
                    transfers: transfers.into_iter().map(|v| v.into()).collect(),
                    redirect_to,
                };
                proto::alkanes::alkanes_trace_event::Event::ValueTransfer(value_transfer)
            }
            TraceEvent::EnterCall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = 1;
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterStaticcall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = 3;
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::EnterDelegatecall(v) => {
                let mut context: proto::alkanes::AlkanesEnterContext = v.into();
                context.call_type = 2;
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context)
            }
            TraceEvent::ReturnContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = 0;
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::RevertContext(v) => {
                let mut context: proto::alkanes::AlkanesExitContext = v.into();
                context.status = 1;
                proto::alkanes::alkanes_trace_event::Event::ExitContext(context)
            }
            TraceEvent::CreateAlkane(v) => {
                let creation = proto::alkanes::AlkanesCreate {
                    new_alkane: Some(v.into()),
                };
                proto::alkanes::alkanes_trace_event::Event::CreateAlkane(creation)
            }
        };
        proto::alkanes::AlkanesTraceEvent { event: Some(event) }
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
            myself: v.myself.map_or(AlkaneId::default(), |v| v.into()),
            caller: v.caller.map_or(AlkaneId::default(), |v| v.into()),
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
        TraceResponse {
            inner: v
                .response
                .map_or(ExtendedCallResponse::default(), |v| v.into()),
            fuel_used: 0,
        }
    }
}

impl From<proto::alkanes::TraceContext> for TraceContext {
    fn from(v: proto::alkanes::TraceContext) -> Self {
        Self {
            inner: v.inner.map_or(Context::default(), |v| v.into()),
            fuel: v.fuel.into(),
            target: AlkaneId::default(),
        }
    }
}

impl From<proto::alkanes::AlkanesEnterContext> for TraceContext {
    fn from(v: proto::alkanes::AlkanesEnterContext) -> TraceContext {
        let mut context: TraceContext = v.context.map_or(TraceContext::default(), |v| v.into());
        context.target = match v.call_type {
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
        if let Some(event) = v.event {
            match event {
                proto::alkanes::alkanes_trace_event::Event::EnterContext(context) => {
                    match context.call_type {
                        1 => TraceEvent::EnterCall(context.into()),
                        2 => TraceEvent::EnterDelegatecall(context.into()),
                        3 => TraceEvent::EnterStaticcall(context.into()),
                        _ => TraceEvent::EnterCall(context.into()),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::ExitContext(v) => match v.status {
                    0 => TraceEvent::ReturnContext(
                        v.response.map_or(Default::default(), |r| r.into()),
                    ),
                    _ => TraceEvent::RevertContext(
                        v.response.map_or(Default::default(), |r| r.into()),
                    ),
                },
                proto::alkanes::alkanes_trace_event::Event::CreateAlkane(v) => {
                    TraceEvent::CreateAlkane(v.new_alkane.map_or(Default::default(), |a| a.into()))
                }
                proto::alkanes::alkanes_trace_event::Event::ReceiveIntent(v) => {
                    TraceEvent::ReceiveIntent {
                        incoming_alkanes: AlkaneTransferParcel(
                            v.incoming_alkanes
                                .into_iter()
                                .map(|v| v.into())
                                .collect::<Vec<AlkaneTransfer>>(),
                        ),
                    }
                }
                proto::alkanes::alkanes_trace_event::Event::ValueTransfer(v) => {
                    TraceEvent::ValueTransfer {
                        transfers: v.transfers.into_iter().map(|v| v.into()).collect(),
                        redirect_to: v.redirect_to,
                    }
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
        proto::alkanes::AlkanesTrace {
            events: self
                .into_iter()
                .map(|v| v.into())
                .collect::<Vec<proto::alkanes::AlkanesTraceEvent>>(),
        }
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
        Ok(proto::alkanes::AlkanesTrace::decode(v.as_ref())?.into())
    }
}
