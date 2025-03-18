use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::{
    context::Context, id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse,
};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;

#[derive(Default)]
pub struct GenesisProtorune(());

#[derive(MessageDispatch)]
enum GenesisProtoruneMessage {
    #[opcode(0)]
    Initialize,

    #[opcode(77)]
    Mint,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,
}

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
            panic!("can only mint in response to incoming QUORUM•GENESIS•PROTORUNE");
        }
        let value = context.incoming_alkanes.0[0].value;
        let mut total_supply_pointer = self.total_supply_pointer();
        total_supply_pointer.set_value::<u128>(total_supply_pointer.get_value::<u128>() + value);
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }

    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // No initialization logic

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
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

impl AlkaneResponder for GenesisProtorune {
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
    impl AlkaneResponder for GenesisProtorune {
        type Message = GenesisProtoruneMessage;
    }
}
