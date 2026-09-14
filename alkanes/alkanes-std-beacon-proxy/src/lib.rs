#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use alkanes_macros::storage_variable;
use alkanes_runtime::auth::AuthenticatedResponder;
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

use generated::BeaconProxyInterface;

#[derive(Default)]
pub struct BeaconProxy(());

impl BeaconProxy {
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

impl BeaconProxyInterface for BeaconProxy {
    fn initialize(&self, beacon: AlkaneId) -> Result<CallResponse> {
        self.observe_proxy_initialization()?;
        let context = self.context()?;

        self.set_beacon(beacon);
        let response: CallResponse = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }

    fn forward(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
}
