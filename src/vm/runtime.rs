use std::fmt;

use alkanes_support::{
    cellpack::Cellpack, context::Context, id::AlkaneId, parcel::AlkaneTransferParcel, trace::Trace,
};
#[allow(unused_imports)]
use {
    metashrew::{println, stdio::stdout},
    std::fmt::Write,
};

use protorune::message::MessageContextParcel;

#[derive(Default, Clone)]
pub struct AlkanesRuntimeContext {
    pub myself: AlkaneId,
    pub caller: AlkaneId,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub returndata: Vec<u8>,
    pub inputs: Vec<u128>,
    pub message: Box<MessageContextParcel>,
    pub trace: Trace,
}

impl fmt::Debug for AlkanesRuntimeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlkanesRuntimeContext")
            .field("myself", &self.myself)
            .field("caller", &self.caller)
            .field("incoming_alkanes", &self.incoming_alkanes)
            .field("inputs", &self.inputs)
            .finish()
    }
}

impl AlkanesRuntimeContext {
    pub fn from_parcel_and_cellpack(
        message: &MessageContextParcel,
        cellpack: &Cellpack,
    ) -> AlkanesRuntimeContext {
        let cloned = cellpack.clone();
        let message_copy = message.clone();
        let incoming_alkanes = message_copy.runes.clone().into();
        AlkanesRuntimeContext {
            message: Box::new(message_copy),
            returndata: vec![],
            incoming_alkanes,
            myself: AlkaneId::default(),
            caller: AlkaneId::default(),
            trace: Trace::default(),
            inputs: cloned.inputs,
        }
    }
    pub fn to_context(&self) -> Context {
      <Self as Into<Context>>::into(self.clone())
    }
    pub fn flatten(&self) -> Vec<u128> {
        let mut result = Vec::<u128>::new();
        result.push(self.myself.block);
        result.push(self.myself.tx);
        result.push(self.caller.block);
        result.push(self.caller.tx);
        result.push(self.message.vout as u128);
        result.push(self.incoming_alkanes.0.len() as u128);
        for incoming in &self.incoming_alkanes.0 {
            result.push(incoming.id.block);
            result.push(incoming.id.tx);
            result.push(incoming.value);
        }
        for input in self.inputs.clone() {
            result.push(input);
        }
        result
    }
    pub fn serialize(&self) -> Vec<u8> {
        let result = self
            .flatten()
            .into_iter()
            .map(|v| {
                let ar = (&v.to_le_bytes()).to_vec();
                ar
            })
            .flatten()
            .collect::<Vec<u8>>();
        result
    }
}

impl From<AlkanesRuntimeContext> for Context {
    fn from(value: AlkanesRuntimeContext) -> Self {
        Context {
            myself: value.myself,
            caller: value.caller,
            vout: value.message.vout,
            incoming_alkanes: value.incoming_alkanes,
            inputs: value.inputs,
        }
    }
}

impl From<Context> for AlkanesRuntimeContext {
    fn from(value: Context) -> Self {
        AlkanesRuntimeContext {
            myself: value.myself,
            caller: value.caller,
            incoming_alkanes: value.incoming_alkanes,
            inputs: value.inputs,
            message: Box::new(MessageContextParcel {
                vout: value.vout,
                ..Default::default()
            }),
            returndata: vec![],
            trace: Trace::default(),
        }
    }
}
