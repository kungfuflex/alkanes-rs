use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{
    runtime::{AlkaneEnvironment, AlkaneResponder},
    storage::StoragePointer,
    token::Token,
};
use alkanes_support::utils::overflow_error;
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use bitcoin::Block;
use hex;
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use std::io::Cursor;
pub mod chain;
use crate::chain::ChainConfiguration;

#[derive(Default)]
pub struct GenesisAlkane {
    pub env: AlkaneEnvironment,
}

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
    fn block_reward(&mut self, n: u64) -> u128 {
        return (50e8 as u128) / (1u128 << ((n as u128) / 210000u128));
    }
    fn genesis_block(&mut self) -> u64 {
        0
    }
    fn premine(&mut self) -> Result<u128> {
        Ok(50_000_000)
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        50_000_000
    }
    fn max_supply(&mut self) -> u128 {
        u128::MAX
    }
}

#[cfg(feature = "mainnet")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&mut self, n: u64) -> u128 {
        return (50e8 as u128) / (1u128 << ((n as u128) / 210000u128));
    }
    fn genesis_block(&mut self) -> u64 {
        800000
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        468750000
    }
    fn max_supply(&mut self) -> u128 {
        156250000000000
    }
}

#[cfg(feature = "dogecoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&mut self, n: u64) -> u128 {
        1_000_000_000_000u128
    }
    fn genesis_block(&mut self) -> u64 {
        4_000_000u64
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        1_000_000_000_000u128
    }
    fn max_supply(&mut self) -> u128 {
        4_000_000_000_000_000_000u128
    }
}

#[cfg(feature = "fractal")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&mut self, n: u64) -> u128 {
        return (25e8 as u128) / (1u128 << ((n as u128) / 2100000u128));
    }
    fn genesis_block(&mut self) -> u64 {
        0e64
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        2_500_000_000
    }
    fn max_supply(&mut self) -> u128 {
        21_000_000_000_000_000
    }
}

#[cfg(feature = "luckycoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&mut self, n: u64) -> u128 {
        1_000_000_000
    }
    fn genesis_block(&mut self) -> u64 {
        0e64
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        1_000_000_000
    }
    fn max_supply(&mut self) -> u128 {
        20e14
    }
}

#[cfg(feature = "bellscoin")]
impl ChainConfiguration for GenesisAlkane {
    fn block_reward(&mut self, n: u64) -> u128 {
        1_000_000_000
    }
    fn genesis_block(&mut self) -> u64 {
        0u64
    }
    fn average_payout_from_genesis(&mut self) -> u128 {
        1_000_000_000
    }
    fn max_supply(&mut self) -> u128 {
        20e14 as u128
    }
}

impl GenesisAlkane {

    pub fn seen_pointer(&mut self, hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/seen/").select(&hash)
    }

    pub fn hash(&mut self, block: &Block) -> Vec<u8> {
        block.block_hash().as_byte_array().to_vec()
    }

    pub fn total_supply_pointer(&mut self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&mut self) -> u128 {
        self.total_supply_pointer()
            .get_value::<u128>(&mut self.env)
    }

    pub fn increase_total_supply(&mut self, v: u128) -> Result<()> {
        let total_supply = self.total_supply();
        self.set_total_supply(overflow_error(total_supply.checked_add(v))?);
        Ok(())
    }

    pub fn set_total_supply(&mut self, v: u128) {
        self.total_supply_pointer()
            .set_value::<u128>(&mut self.env, v);
    }

    pub fn observe_mint(&mut self) -> Result<()> {
        let hash = self.height().to_le_bytes().to_vec();
        let mut pointer = self.seen_pointer(&hash);
        if pointer.get(&mut self.env).len() == 0 {
            pointer.set_value::<u32>(&mut self.env, 1);
            Ok(())
        } else {
            Err(anyhow!(format!(
                "already minted for block {}",
                hex::encode(&hash)
            )))
        }
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&mut self) -> Result<AlkaneTransfer> {
        self.observe_mint()?;
        let value = self.current_block_reward();
        let mut total_supply_pointer = self.total_supply_pointer();
        let total_supply = total_supply_pointer.get_value::<u128>(&mut self.env);
        if total_supply >= self.max_supply() {
            return Err(anyhow!("total supply has been reached"));
        }
        total_supply_pointer.set_value::<u128>(&mut self.env, total_supply + value);
        Ok(AlkaneTransfer {
            id: self.context()?.myself.clone(),
            value,
        })
    }

    fn initialize(&mut self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);

        self.observe_mint()?;
        let premine = self.premine()?;
        response.alkanes.0.push(AlkaneTransfer {
            id: self.context()?.myself.clone(),
            value: premine,
        });
        self.increase_total_supply(premine)?;

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&mut self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);

        response.alkanes.0.push(self.create_mint_transfer()?);

        Ok(response)
    }

    fn get_name(&mut self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);

        response.data = self.name().into_bytes().to_vec();

        Ok(response)
    }

    fn get_symbol(&mut self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);

        response.data = self.symbol().into_bytes().to_vec();

        Ok(response)
    }

    fn get_total_supply(&mut self) -> Result<CallResponse> {
        let mut response = CallResponse::forward(&self.context()?.incoming_alkanes);

        response.data = (&self.total_supply().to_le_bytes()).to_vec();

        Ok(response)
    }
}

impl AlkaneResponder for GenesisAlkane {
    fn env(&mut self) -> &mut AlkaneEnvironment {
        &mut self.env
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for GenesisAlkane {
        type Message = GenesisAlkaneMessage;
    }
}
