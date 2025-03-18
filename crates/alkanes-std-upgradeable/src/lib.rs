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
        block: u128,
        tx: u128,
        auth_token_units: u128,
    },

    #[opcode(0x7ffe)]
    Upgrade { block: u128, tx: u128 },

    #[opcode(0x7ffd)]
    Delegate,
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

    fn initialize(&self, block: u128, tx: u128, auth_token_units: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut pointer = StoragePointer::from_keyword("/proxy-initialized");

        if pointer.get().len() == 0 {
            // Construct AlkaneId from block and tx
            let implementation = AlkaneId::new(block, tx);

            self.set_alkane(implementation);
            let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes);

            response
                .alkanes
                .0
                .push(self.deploy_auth_token(auth_token_units)?);
            pointer.set(Arc::new(vec![0x01]));
            Ok(response)
        } else {
            Err(anyhow!("already initialized"))
        }
    }

    fn upgrade(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;

        self.only_owner()?;

        // Construct AlkaneId from block and tx
        let implementation = AlkaneId::new(block, tx);

        self.set_alkane(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn delegate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: self.alkane()?,
            inputs: context.inputs.clone(),
        };
        Ok(self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())?)
    }
}

impl AuthenticatedResponder for Upgradeable {}

impl AlkaneResponder for Upgradeable {
    fn execute(&self) -> Result<CallResponse> {
        // The opcode extraction and dispatch logic is now handled by the declare_alkane macro
        // This method is still required by the AlkaneResponder trait, but we can just return an error
        // indicating that it should not be called directly
        Err(anyhow!(
            "This method should not be called directly. Use the declare_alkane macro instead."
        ))
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for Upgradeable {
        type Message = UpgradeableMessage;
    }
}
