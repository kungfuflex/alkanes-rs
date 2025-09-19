use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::{AlkaneResponder, AlkaneEnvironment}, storage::StoragePointer, token::Token};
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::Result;
use hex_lit::hex;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

pub struct Orbital {
	env: AlkaneEnvironment,
}

impl Default for Orbital {
    fn default() -> Self {
        Self {
            env: AlkaneEnvironment::new(),
        }
    }
}

#[derive(MessageDispatch)]
enum OrbitalMessage {
    #[opcode(0)]
    Initialize,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    #[opcode(1000)]
    #[returns(Vec<u8>)]
    GetData,
}

impl Token for Orbital {
    fn name(&self) -> String {
        String::from("NFT")
    }
    fn symbol(&self) -> String {
        String::from("NFT")
    }
}

impl Orbital {
    pub fn total_supply_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&mut self) -> u128 {
        Self::total_supply_pointer().get_value::<u128>(self.env())
    }

    pub fn set_total_supply(&mut self, v: u128) {
        Self::total_supply_pointer().set_value::<u128>(self.env(), v);
    }

    pub fn data(&self) -> Vec<u8> {
        // in this reference implementation, we return a 1x1 PNG
        // NFT data can be anything, however
        (&hex!("89504e470d0a1a0a0000000d494844520000000100000001010300000025db56ca00000003504c5445000000a77a3dda0000000174524e530040e6d8660000000a4944415408d76360000000020001e221bc330000000049454e44ae426082")).to_vec()
    }

    fn initialize(&mut self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.set_total_supply(1);
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });

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
        let total_supply = self.total_supply();
        response.data = (&total_supply.to_le_bytes()).to_vec();

        Ok(response)
    }

    fn get_data(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.data();

        Ok(response)
    }
}

impl AlkaneResponder for Orbital {
    fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.env
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for Orbital {
        type Message = OrbitalMessage;
    }
}
