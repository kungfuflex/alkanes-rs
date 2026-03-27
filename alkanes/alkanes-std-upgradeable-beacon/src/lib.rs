#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use alkanes_runtime::auth::AuthenticatedResponder;
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

use generated::UpgradeableBeaconInterface;

#[derive(Default)]
pub struct UpgradeableBeacon(());

impl UpgradeableBeacon {
    pub fn implementation_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/implementation")
    }

    pub fn _implementation() -> Result<AlkaneId> {
        Ok(Self::implementation_pointer()
            .get()
            .as_ref()
            .clone()
            .try_into()?)
    }

    pub fn set_implementation(v: AlkaneId) {
        Self::implementation_pointer().set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(v)));
    }
}

impl AuthenticatedResponder for UpgradeableBeacon {}

impl AlkaneResponder for UpgradeableBeacon {}

impl UpgradeableBeaconInterface for UpgradeableBeacon {
    fn initialize(&self, implementation: AlkaneId, auth_token_units: u128) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;

        Self::set_implementation(implementation);
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.deploy_auth_token(auth_token_units)?);
        Ok(response)
    }

    fn implementation(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = Self::_implementation()?.into();
        Ok(response)
    }

    fn upgrade_to(&self, implementation: AlkaneId) -> Result<CallResponse> {
        let context = self.context()?;

        self.only_owner()?;

        Self::set_implementation(implementation);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn forward(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
}
