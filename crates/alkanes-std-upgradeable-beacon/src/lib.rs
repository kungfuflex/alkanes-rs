use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{environment::AlkaneEnvironment, runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{id::AlkaneId, response::CallResponse};
use anyhow::Result;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

pub struct UpgradeableBeacon {
    env: AlkaneEnvironment,
}

impl Default for UpgradeableBeacon {
    fn default() -> Self {
        Self {
            env: AlkaneEnvironment::new(),
        }
    }
}

#[derive(MessageDispatch)]
enum UpgradeableBeaconMessage {
    #[opcode(0x7fff)]
    Initialize {
        implementation: AlkaneId,
        auth_token_units: u128,
    },

    #[opcode(0x7ffd)]
    Implementation {},

    #[opcode(0x7ffe)]
    UpgradeTo { implementation: AlkaneId },
    #[opcode(0x8fff)]
    Forward {},
}

impl UpgradeableBeacon {
    fn forward(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
    pub fn implementation_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/implementation")
    }

    pub fn _implementation(&mut self) -> Result<AlkaneId> {
        Ok(Self::implementation_pointer()
            .get(self.env())
            .as_ref()
            .clone()
            .try_into()?)
    }

    pub fn set_implementation(&mut self, v: AlkaneId) {
        Self::implementation_pointer().set(self.env(), Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }

    fn initialize(&mut self, implementation: AlkaneId, auth_token_units: u128) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;

        self.set_implementation(implementation);
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.deploy_auth_token(auth_token_units)?);
        Ok(response)
    }

    fn implementation(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self._implementation()?.into();
        Ok(response)
    }

    fn upgrade_to(&mut self, implementation: AlkaneId) -> Result<CallResponse> {
        let context = self.context()?;

        self.only_owner()?;

        self.set_implementation(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
}

impl AuthenticatedResponder for UpgradeableBeacon {}

impl AlkaneResponder for UpgradeableBeacon {
    fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.env
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for UpgradeableBeacon {
        type Message = UpgradeableBeaconMessage;
    }
}
