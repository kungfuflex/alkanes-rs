use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
use alkanes_runtime::environment::AlkaneEnvironment;
use alkanes_runtime::message::MessageDispatch;
use metashrew_support::compat::to_arraybuffer_layout;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{cellpack::Cellpack, id::AlkaneId, response::CallResponse};
use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Default)]
pub struct Upgradeable(AlkaneEnvironment);

#[derive(MessageDispatch)]
enum UpgradeableMessage {
    #[opcode(0x7fff)]
    Initialize {
        implementation: AlkaneId,
        auth_token_units: u128,
    },

    #[opcode(0x7ffe)]
    Upgrade { implementation: AlkaneId },
    #[opcode(0x8fff)]
    Forward {},
}

impl Upgradeable {
    fn forward(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
    pub fn alkane_pointer(&mut self) -> StoragePointer {
        StoragePointer::from_keyword("/implementation")
    }

    pub fn alkane(&mut self) -> Result<AlkaneId> {
        Ok(self.alkane_pointer().get(&mut self.0).as_ref().clone().try_into()?)
    }

    pub fn set_alkane(&mut self, v: AlkaneId) {
        self.alkane_pointer().set(&mut self.0, Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }

    fn initialize(&mut self, implementation: AlkaneId, auth_token_units: u128) -> Result<CallResponse> {
        self.observe_proxy_initialization()?;
        let context = self.context()?;

        self.set_alkane(implementation);
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.deploy_auth_token(auth_token_units)?);
        Ok(response)
    }

    fn upgrade(&mut self, implementation: AlkaneId) -> Result<CallResponse> {
        let context = self.context()?;

        self.only_owner()?;

        self.set_alkane(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
}

impl AuthenticatedResponder for Upgradeable {}

impl AlkaneResponder for Upgradeable {
    fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.0
    }

    fn fallback(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let inputs: Vec<u128> = context.inputs.clone();
        let cellpack = Cellpack {
            target: self.alkane()?,
            inputs: inputs,
        };
        self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for Upgradeable {
        type Message = UpgradeableMessage;
    }
}
