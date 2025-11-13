use alkanes_macros::storage_variable;
use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
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
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Default)]
pub struct BeaconProxy(());

#[derive(MessageDispatch)]
enum BeaconProxyMessage {
    #[opcode(0x7fff)]
    Initialize { beacon: AlkaneId },
    #[opcode(0x8fff)]
    Forward {},
}

impl BeaconProxy {
    fn forward(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }

    storage_variable!(beacon: AlkaneId);

    pub fn get_logic_impl(&self) -> Result<AlkaneId> {
        let beacon = self.beacon()?;
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

    fn initialize(&self, implementation: AlkaneId) -> Result<CallResponse> {
        self.observe_proxy_initialization()?;
        let context = self.context()?;

        self.set_beacon(implementation);
        let response: CallResponse = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
}

impl AuthenticatedResponder for BeaconProxy {}

impl AlkaneResponder for BeaconProxy {
    fn fallback(&self) -> Result<CallResponse> {
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
