use serde::{Deserialize, Serialize};
use alkanes_support::id::AlkaneId;
use bitcoin::OutPoint;
use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesTrace {
    pub events: Vec<AlkanesTraceEvent>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesTraceEvent {
    pub event: Option<AlkanesTraceEvent_oneof_event>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub enum AlkanesTraceEvent_oneof_event {
    EnterContext(AlkanesEnterContext),
    ExitContext(AlkanesExitContext),
    CreateAlkane(AlkanesCreate),
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesEnterContext {
    pub call_type: AlkanesTraceCallType,
    pub context: TraceContext,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesExitContext {
    pub status: AlkanesTraceStatusFlag,
    pub response: ExtendedCallResponse,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesCreate {
    #[serde_as(as = "DisplayFromStr")]
    pub new_alkane: AlkaneId,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub inner: Context,
    pub fuel: u64,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct Context {
    #[serde_as(as = "DisplayFromStr")]
    pub myself: AlkaneId,
    #[serde_as(as = "DisplayFromStr")]
    pub caller: AlkaneId,
    pub inputs: Vec<u128>,
    pub vout: u32,
    pub incoming_alkanes: Vec<AlkaneTransfer>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct ExtendedCallResponse {
    pub alkanes: Vec<AlkaneTransfer>,
    pub storage: Vec<KeyValuePair>,
    #[serde_as(as = "serde_with::hex::Hex")]
    pub data: Vec<u8>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyValuePair {
    #[serde_as(as = "serde_with::hex::Hex")]
    pub key: Vec<u8>,
    #[serde_as(as = "serde_with::hex::Hex")]
    pub value: Vec<u8>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkaneTransfer {
    #[serde_as(as = "DisplayFromStr")]
    pub id: AlkaneId,
    pub value: u128,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlkanesTraceCallType {
    NONE = 0,
    CALL = 1,
    DELEGATECALL = 2,
    STATICCALL = 3,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlkanesTraceStatusFlag {
    SUCCESS = 0,
    FAILURE = 1,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesBlockEvent {
    pub traces: AlkanesTrace,
    #[serde_as(as = "DisplayFromStr")]
    pub outpoint: OutPoint,
    pub txindex: u64,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct AlkanesBlockTraceEvent {
    pub events: Vec<AlkanesBlockEvent>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct Trace {
    #[serde_as(as = "DisplayFromStr")]
    pub outpoint: OutPoint,
    pub trace: AlkanesTrace,
}

use alkanes_support::proto;


// From proto to serde
impl From<proto::alkanes::AlkanesTrace> for AlkanesTrace {
    fn from(proto_trace: proto::alkanes::AlkanesTrace) -> Self {
        Self {
            events: proto_trace.events.into_iter().map(|e| e.into()).collect(),
        }
    }
}

impl From<proto::alkanes::AlkanesTraceEvent> for AlkanesTraceEvent {
    fn from(proto_event: proto::alkanes::AlkanesTraceEvent) -> Self {
        Self {
            event: proto_event.event.map(|e| e.into()),
        }
    }
}



impl From<proto::alkanes::AlkanesEnterContext> for AlkanesEnterContext {
    fn from(proto_context: proto::alkanes::AlkanesEnterContext) -> Self {
        Self {
            call_type: proto_context.call_type.into(),
            context: proto_context.context.unwrap().into(),
        }
    }
}

impl From<proto::alkanes::AlkanesExitContext> for AlkanesExitContext {
    fn from(proto_context: proto::alkanes::AlkanesExitContext) -> Self {
        Self {
            status: proto_context.status.into(),
            response: proto_context.response.unwrap().into(),
        }
    }
}

impl From<proto::alkanes::AlkanesCreate> for AlkanesCreate {
    fn from(proto_create: proto::alkanes::AlkanesCreate) -> Self {
        Self {
            new_alkane: proto_create.new_alkane.unwrap().into(),
        }
    }
}

impl From<proto::alkanes::TraceContext> for TraceContext {
    fn from(proto_context: proto::alkanes::TraceContext) -> Self {
        Self {
            inner: proto_context.inner.unwrap().into(),
            fuel: proto_context.fuel,
        }
    }
}

impl From<proto::alkanes::Context> for Context {
    fn from(proto_context: proto::alkanes::Context) -> Self {
        Self {
            myself: proto_context.myself.unwrap().into(),
            caller: proto_context.caller.unwrap().into(),
            inputs: proto_context
                .inputs
                .into_iter()
                .map(|i| (i.hi as u128) << 64 | i.lo as u128)
                .collect(),
            vout: proto_context.vout,
            incoming_alkanes: proto_context
                .incoming_alkanes
                .into_iter()
                .map(|t| t.into())
                .collect(),
        }
    }
}

impl From<proto::alkanes::ExtendedCallResponse> for ExtendedCallResponse {
    fn from(proto_response: proto::alkanes::ExtendedCallResponse) -> Self {
        Self {
            alkanes: proto_response
                .alkanes
                .into_iter()
                .map(|t| t.into())
                .collect(),
            storage: proto_response
                .storage
                .into_iter()
                .map(|kv| kv.into())
                .collect(),
            data: proto_response.data,
        }
    }
}

impl From<proto::alkanes::KeyValuePair> for KeyValuePair {
    fn from(proto_kv: proto::alkanes::KeyValuePair) -> Self {
        Self {
            key: proto_kv.key,
            value: proto_kv.value,
        }
    }
}

impl From<proto::alkanes::AlkaneTransfer> for AlkaneTransfer {
    fn from(proto_transfer: proto::alkanes::AlkaneTransfer) -> Self {
        Self {
            id: proto_transfer.id.unwrap().into(),
            value: (proto_transfer.value.as_ref().unwrap().hi as u128) << 64
                | proto_transfer.value.as_ref().unwrap().lo as u128,
        }
    }
}

impl From<i32> for AlkanesTraceCallType {
    fn from(val: i32) -> Self {
        match val {
            0 => AlkanesTraceCallType::NONE,
            1 => AlkanesTraceCallType::CALL,
            2 => AlkanesTraceCallType::DELEGATECALL,
            3 => AlkanesTraceCallType::STATICCALL,
            _ => unreachable!(),
        }
    }
}

impl From<i32> for AlkanesTraceStatusFlag {
    fn from(val: i32) -> Self {
        match val {
            0 => AlkanesTraceStatusFlag::SUCCESS,
            1 => AlkanesTraceStatusFlag::FAILURE,
            _ => unreachable!(),
        }
    }
}

// From serde to proto

impl From<AlkanesTrace> for proto::alkanes::AlkanesTrace {
    fn from(trace: AlkanesTrace) -> Self {
        let mut proto_trace = proto::alkanes::AlkanesTrace::default();
        proto_trace.events = trace.events.into_iter().map(|e| e.into()).collect();
        proto_trace
    }
}

impl From<AlkanesTraceEvent> for proto::alkanes::AlkanesTraceEvent {
    fn from(event: AlkanesTraceEvent) -> Self {
        let mut proto_event = proto::alkanes::AlkanesTraceEvent::default();
        proto_event.event = event.event.map(|e| e.into());
        proto_event
    }
}