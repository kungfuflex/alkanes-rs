use std::cmp::min;

use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::{auth::AuthenticatedResponder, declare_alkane};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::utils::overflow_error;
use alkanes_support::{context::Context, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
pub mod chain;
use crate::chain::ChainConfiguration;

#[derive(Default)]
pub struct GenesisAlkane(());

#[derive(MessageDispatch)]
enum GenesisAlkaneMessage {
    #[opcode(0)]
    Initialize,

    #[opcode(77)]
    Mint,

    #[opcode(78)]
    CollectFees,

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
    pub fn seen_pointer(&self, height: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/seen/").select(&height)
    }

    pub fn claimable_fees_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/fees")
    }

    pub fn claimable_fees(&self) -> u128 {
        self.claimable_fees_pointer().get_value::<u128>()
    }

    pub fn increase_claimable_fees(&self, v: u128) -> Result<()> {
        self.set_claimable_fees(overflow_error(self.claimable_fees().checked_add(v))?);
        Ok(())
    }

    pub fn set_claimable_fees(&self, v: u128) {
        self.claimable_fees_pointer().set_value::<u128>(v);
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

    pub fn observe_mint(&self, diesel_fee: u128) -> Result<()> {
        let height = self.height().to_le_bytes().to_vec();
        let mut pointer = self.seen_pointer(&height);
        if pointer.get().len() == 0 {
            pointer.set_value::<u32>(1);
            if self.claimable_fees_pointer().get().len() == 0 {
                self.set_claimable_fees(0);
            }
            self.increase_claimable_fees(diesel_fee)?;
            self.increase_total_supply(diesel_fee)?;
        }
        Ok(())
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&self) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        let total_mints = self.number_diesel_mints()?;
        let total_miner_fee = self.total_miner_fee()?;
        let block_reward = self.current_block_reward();
        let total_tx_fee = total_miner_fee
            .checked_sub(block_reward)
            .ok_or("")
            .map_err(|_| anyhow!("total miner fee is less than block reward"))?;
        let diesel_fee = min(block_reward / 2, total_tx_fee); // fee is capped at 50% of the block reward
        let value_per_mint = (block_reward - diesel_fee) / total_mints;
        self.observe_mint(diesel_fee)?;

        if self.total_supply() >= self.max_supply() {
            return Err(anyhow!("total supply has been reached"));
        }
        self.increase_total_supply(value_per_mint)?;
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value: value_per_mint,
        })
    }

    fn observe_upgrade_initialization(&self) -> Result<()> {
        let mut pointer = StoragePointer::from_keyword("/upgrade_initialized");
        if pointer.get().len() == 0 {
            pointer.set_value::<u8>(0x01);
            Ok(())
        } else {
            Err(anyhow!("already initialized"))
        }
    }

    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.observe_upgrade_initialization()?;
        response.alkanes.0.push(self.deploy_auth_token(5)?); // hardcode 5 auth tokens

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(self.create_mint_transfer()?);

        Ok(response)
    }

    fn collect_fees(&self) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.alkanes.pay(AlkaneTransfer {
            id: context.myself,
            value: self.claimable_fees(),
        });
        self.set_claimable_fees(0);
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

impl AuthenticatedResponder for GenesisAlkane {}
impl AlkaneResponder for GenesisAlkane {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for GenesisAlkane {
        type Message = GenesisAlkaneMessage;
    }
}
