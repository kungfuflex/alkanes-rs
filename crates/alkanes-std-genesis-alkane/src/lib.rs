use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::utils::overflow_error;
use alkanes_support::{context::Context, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use bitcoin::Block;
use hex;
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::io::Cursor;
pub mod chain;
use crate::chain::{ChainConfiguration, CONTEXT_HANDLE};

#[derive(Default)]
pub struct GenesisAlkane(());

#[derive(MessageDispatch)]
enum GenesisAlkaneMessage {
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

impl Token for GenesisAlkane {
    fn name(&self) -> String {
        String::from("DIESEL")
    }
    fn symbol(&self) -> String {
        String::from("DIESEL")
    }
}

//use if regtest
#[cfg(not(any(
    feature = "mainnet",
    feature = "dogecoin",
    feature = "bellscoin",
    feature = "fractal",
    feature = "luckycoin"
)))]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        return (50e8 as u128) / (1u128 << ((n as u128) / 210000u128));
    }
    fn genesis_block(&self) -> u64 {
        0
    }
    fn average_payout_from_genesis(&self) -> u128 {
        50_000_000
    }
    fn max_supply(&self) -> u128 {
        u128::MAX
    }
}

#[cfg(feature = "mainnet")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        return (50e8 as u128) / (1u128 << ((n as u128) / 210000u128));
    }
    fn genesis_block(&self) -> u64 {
        800000
    }
    fn average_payout_from_genesis(&self) -> u128 {
        468750000
    }
    fn max_supply(&self) -> u128 {
        156250000000000
    }
}

#[cfg(feature = "dogecoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        1_000_000_000_000u128
    }
    fn genesis_block(&self) -> u64 {
        4_000_000u64
    }
    fn average_payout_from_genesis(&self) -> u128 {
        1_000_000_000_000u128
    }
    fn max_supply(&self) -> u128 {
        4_000_000_000_000_000_000u128
    }
}

#[cfg(feature = "fractal")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        return (25e8 as u128) / (1u128 << ((n as u128) / 2100000u128));
    }
    fn genesis_block(&self) -> u64 {
        0e64
    }
    fn average_payout_from_genesis(&self) -> u128 {
        2_500_000_000
    }
    fn max_supply(&self) -> u128 {
        21_000_000_000_000_000
    }
}

#[cfg(feature = "luckycoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        1_000_000_000
    }
    fn genesis_block(&self) -> u64 {
        0e64
    }
    fn average_payout_from_genesis(&self) -> u128 {
        1_000_000_000
    }
    fn max_supply(&self) -> u128 {
        20e14
    }
}

#[cfg(feature = "bellscoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&self, n: u64) -> u128 {
        1_000_000_000
    }
    fn genesis_block(&self) -> u64 {
        0u64
    }
    fn average_payout_from_genesis(&self) -> u128 {
        1_000_000_000
    }
    fn max_supply(&self) -> u128 {
        20e14 as u128
    }
}

impl GenesisAlkane {
    fn block(&self) -> Result<Block> {
        Ok(AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(CONTEXT_HANDLE.block()))?.to_consensus())
    }

    pub fn seen_pointer(&self, hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/seen/").select(&hash)
    }

    pub fn hash(&self, block: &Block) -> Vec<u8> {
        block.block_hash().as_byte_array().to_vec()
    }

    pub fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
    }

    pub fn increase_total_supply(&self, v: u128) -> Result<()> {
        self.set_total_supply(overflow_error(self.total_supply().checked_add(v))?);
        Ok(())
    }

    pub fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }

    pub fn observe_mint(&self) -> Result<()> {
        let hash = self.height().to_le_bytes().to_vec();
        let mut pointer = self.seen_pointer(&hash);
        if pointer.get().len() == 0 {
            pointer.set_value::<u32>(1);
            Ok(())
        } else {
            Err(anyhow!(format!(
                "already minted for block {}",
                hex::encode(&hash)
            )))
        }
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&self) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        self.observe_mint()?;
        let value = self.current_block_reward();
        let mut total_supply_pointer = self.total_supply_pointer();
        let total_supply = total_supply_pointer.get_value::<u128>();
        if total_supply >= self.max_supply() {
            return Err(anyhow!("total supply has been reached"));
        }
        total_supply_pointer.set_value::<u128>(total_supply + value);
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }

    pub fn observe_initialization(&self) -> Result<()> {
        self.observe_mint()?;
        let mut initialized_pointer = StoragePointer::from_keyword("/initialized");
        if initialized_pointer.get().len() == 0 {
            initialized_pointer.set_value::<u32>(1);
            Ok(())
        } else {
            Err(anyhow!("already initialized"))
        }
    }

    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.observe_initialization()?;
        let premine = self.premine()?;
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: premine,
        });
        self.increase_total_supply(premine)?;

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(self.create_mint_transfer()?);

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

impl AlkaneResponder for GenesisAlkane {
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
    impl AlkaneResponder for GenesisAlkane {
        type Message = GenesisAlkaneMessage;
    }
}
