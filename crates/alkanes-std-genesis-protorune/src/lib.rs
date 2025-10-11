use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token, environment::AlkaneEnvironment};
use alkanes_support::{
    context::Context, id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse,
};
use anyhow::Result;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

#[derive(Default)]
pub struct GenesisProtorune {
	env: AlkaneEnvironment,
}

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

    pub fn total_supply(&mut self) -> u128 {
        self.total_supply_pointer().get_value::<u128>(self.env())
    }

    pub fn set_total_supply(&mut self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(self.env(), v);
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&mut self, context: &Context) -> Result<AlkaneTransfer> {
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
		let total_supply = total_supply_pointer.get_value::<u128>(self.env());
        total_supply_pointer.set_value::<u128>(self.env(), total_supply + value);
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }

    fn initialize(&mut self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);

        // No initialization logic

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response
            .alkanes
            .0
            .push(self.create_mint_transfer(&context)?);

        Ok(response)
    }

    fn get_name(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.name().into_bytes().to_vec();

        Ok(response)
    }

    fn get_symbol(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.symbol().into_bytes().to_vec();

        Ok(response)
    }

    fn get_total_supply(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = (&self.total_supply().to_le_bytes()).to_vec();

        Ok(response)
    }
}

impl AlkaneResponder for GenesisProtorune {
	fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.env
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for GenesisProtorune {
        type Message = GenesisProtoruneMessage;
    }
}