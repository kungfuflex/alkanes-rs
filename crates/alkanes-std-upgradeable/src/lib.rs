use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, compat::to_arraybuffer_layout};
use alkanes_support::{cellpack::Cellpack, id::AlkaneId, response::CallResponse};
use anyhow::{Result};
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
    #[opcode(0x8fff)]
    Forward {},
}

impl Upgradeable {
    fn forward(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
    pub fn alkane_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/implementation")
    }

    pub fn alkane() -> Result<AlkaneId> {
        Ok(Self::alkane_pointer().get().as_ref().clone().try_into()?)
    }

    pub fn set_alkane(v: AlkaneId) {
        Self::alkane_pointer().set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }

    fn initialize(&self, implementation: AlkaneId, auth_token_units: u128) -> Result<CallResponse> {
        self.observe_proxy_initialization()?;
        let context = self.context()?;

        Self::set_alkane(implementation);
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

        Self::set_alkane(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
}

impl AuthenticatedResponder for Upgradeable {}

impl AlkaneResponder for Upgradeable {
    fn fallback(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let inputs: Vec<u128> = context.inputs.clone();
        let cellpack = Cellpack {
            target: Self::alkane()?,
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
