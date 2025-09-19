use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
use alkanes_runtime::environment::AlkaneEnvironment;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{
    cellpack::Cellpack, id::AlkaneId, parcel::AlkaneTransferParcel, response::CallResponse,
};
use anyhow::Result;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Default)]
pub struct BeaconProxy(AlkaneEnvironment);

#[derive(MessageDispatch)]
enum BeaconProxyMessage {
    #[opcode(0x7fff)]
    Initialize { beacon: AlkaneId },
    #[opcode(0x8fff)]
    Forward {},
}

impl BeaconProxy {
    fn forward(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
    pub fn beacon_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/beacon ")
    }

    pub fn beacon(&mut self) -> Result<AlkaneId> {
        Ok(Self::beacon_pointer()
            .get(&mut self.0)
            .as_ref()
            .clone()
            .try_into()?)
    }

    pub fn set_beacon(&mut self, v: AlkaneId) {
        Self::beacon_pointer().set(&mut self.0, Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }

    pub fn get_logic_impl(&mut self) -> Result<AlkaneId> {
        let beacon = Self::beacon_pointer().get(&mut self.0).as_ref().clone().try_into()?;
        let response = self.staticcall(
            &Cellpack {
                target: beacon,
                inputs: vec![0x7ffd],
            },
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        Ok(response.data.try_into()?)
    }

    fn initialize(&mut self, implementation: AlkaneId) -> Result<CallResponse> {
        self.observe_proxy_initialization()?;
        let context = self.context()?;

        self.set_beacon(implementation);
        let response: CallResponse = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
}

impl AuthenticatedResponder for BeaconProxy {}

impl AlkaneResponder for BeaconProxy {
    fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.0
    }
    fn fallback(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let inputs: Vec<u128> = context.inputs.clone();
        let cellpack = Cellpack {
            target: self.get_logic_impl()?,
            inputs: inputs,
        };
        self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for BeaconProxy {
        type Message = BeaconProxyMessage;
    }
}
