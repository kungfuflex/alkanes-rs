use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{cellpack::Cellpack, id::AlkaneId, response::CallResponse};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Default)]
pub struct Upgradeable(());

#[derive(MessageDispatch)]
enum UpgradeableMessage {
    #[opcode(0x7fff)]
    Initialize {
        implementation: AlkaneId,
        auth_token_units: u128,
    },

    #[opcode(0x7ffe)]
    Upgrade { implementation: AlkaneId },

    #[opcode(0x7ffd)]
    Delegate { inputs: Vec<u128> },
}

impl Upgradeable {
    pub fn alkane_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/implementation")
    }

    pub fn alkane(&self) -> Result<AlkaneId> {
        Ok(self.alkane_pointer().get().as_ref().clone().try_into()?)
    }

    pub fn set_alkane(&self, v: AlkaneId) {
        self.alkane_pointer()
            .set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }

    fn initialize(&self, implementation: AlkaneId, auth_token_units: u128) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;

        self.set_alkane(implementation);
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.deploy_auth_token(auth_token_units)?);
        Ok(response)
    }

    fn upgrade(&self, implementation: AlkaneId) -> Result<CallResponse> {
        let context = self.context()?;

        self.only_owner()?;

        self.set_alkane(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn delegate(&self, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: self.alkane()?,
            inputs: inputs,
        };
        self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())
    }
}

impl AuthenticatedResponder for Upgradeable {}

impl AlkaneResponder for Upgradeable {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for Upgradeable {
        type Message = UpgradeableMessage;
    }
}
