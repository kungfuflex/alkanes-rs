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
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::Result;
use hex_lit::hex;
use metashrew_support::index_pointer::KeyValuePointer;

use generated::OrbitalInterface;

#[derive(Default)]
pub struct Orbital(());

impl Token for Orbital {
    fn name(&self) -> String {
        String::from("NFT")
    }
    fn symbol(&self) -> String {
        String::from("NFT")
    }
}

impl Orbital {
    pub fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
    }

    pub fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }

    pub fn data(&self) -> Vec<u8> {
        // in this reference implementation, we return a 1x1 PNG
        // NFT data can be anything, however
        (&hex!("89504e470d0a1a0a0000000d494844520000000100000001010300000025db56ca00000003504c5445000000a77a3dda0000000174524e530040e6d8660000000a4944415408d76360000000020001e221bc330000000049454e44ae426082")).to_vec()
    }
}

impl AlkaneResponder for Orbital {}

impl OrbitalInterface for Orbital {
    fn initialize(&self) -> Result<CallResponse> {
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

    fn get_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = self.data();

        Ok(response)
    }
}
