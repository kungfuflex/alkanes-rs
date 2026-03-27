// Include the generated code from WIT codegen
#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::{
    context::Context, id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse,
};
use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;

use generated::GenesisProtoruneInterface;

#[derive(Default)]
pub struct GenesisProtorune(());

impl Token for GenesisProtorune {
    fn name(&self) -> String {
        String::from("Genesis Protorune")
    }
    fn symbol(&self) -> String {
        String::from("aGP")
    }
}

impl GenesisProtorune {
    pub fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
    }

    pub fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&self, context: &Context) -> Result<AlkaneTransfer> {
        if context.incoming_alkanes.0.len() != 1
            || &context.incoming_alkanes.0[0].id
                != &(AlkaneId {
                    block: 849236,
                    tx: 298,
                })
        {
            panic!("can only mint in response to incoming QUORUM\u{2022}GENESIS\u{2022}PROTORUNE");
        }
        let value = context.incoming_alkanes.0[0].value;
        let mut total_supply_pointer = self.total_supply_pointer();
        total_supply_pointer.set_value::<u128>(total_supply_pointer.get_value::<u128>() + value);
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }
}

impl AlkaneResponder for GenesisProtorune {}

impl GenesisProtoruneInterface for GenesisProtorune {
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // No initialization logic

        Ok(response)
    }

    fn mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.create_mint_transfer(&context)?);

        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.name().into_bytes().to_vec();

        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.symbol().into_bytes().to_vec();

        Ok(response)
    }

    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = (&self.total_supply().to_le_bytes()).to_vec();

        Ok(response)
    }
}
