use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::{auth::AuthenticatedResponder, declare_alkane};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::{AlkaneResponder, AlkaneEnvironment}, storage::StoragePointer, token::Token};
use alkanes_support::utils::overflow_error;
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use bitcoin::{Block, Txid};
use hex;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
pub mod chain;
use crate::chain::ChainConfiguration;

pub struct GenesisAlkane {
	env: AlkaneEnvironment,
}

impl Default for GenesisAlkane {
    fn default() -> Self {
        Self {
            env: AlkaneEnvironment::new(),
        }
    }
}

#[derive(MessageDispatch)]
enum GenesisAlkaneMessage {
    #[opcode(0)]
    Initialize,

    #[opcode(1)]
    Upgrade,

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
    fn premine(&mut self) -> Result<u128> {
        Ok(50_000_000)
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
    fn premine(&mut self) -> Result<u128> {
        Ok(44000000000000)
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
        0
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
        0
    }
    fn average_payout_from_genesis(&self) -> u128 {
        1_000_000_000
    }
    fn max_supply(&self) -> u128 {
        20_000_000_000_000_000
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
        20_000_000_000_000_000
    }
}

impl GenesisAlkane {
    pub fn claimable_fees_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/fees")
    }

    pub fn claimable_fees(&mut self) -> u128 {
        Self::claimable_fees_pointer().get_value::<u128>(self.env())
    }

    pub fn increase_claimable_fees(&mut self, v: u128) -> Result<()> {
        let fees = self.claimable_fees();
        self.set_claimable_fees(overflow_error(fees.checked_add(v))?);
        Ok(())
    }

    pub fn set_claimable_fees(&mut self, v: u128) {
        Self::claimable_fees_pointer().set_value::<u128>(self.env(), v);
    }

    pub fn seen_pointer(hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/seen/").select(&hash)
    }

    pub fn upgraded_seen_pointer(hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/upgraded_seen/").select(&hash)
    }

    pub fn hash(&self, block: &Block) -> Vec<u8> {
        block.block_hash().as_byte_array().to_vec()
    }

    pub fn total_supply_pointer() -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }

    pub fn total_supply(&mut self) -> u128 {
        Self::total_supply_pointer().get_value::<u128>(self.env())
    }

    pub fn increase_total_supply(&mut self, v: u128) -> Result<()> {
        let supply = self.total_supply();
        self.set_total_supply(overflow_error(supply.checked_add(v))?);
        Ok(())
    }

    pub fn set_total_supply(&mut self, v: u128) {
        Self::total_supply_pointer().set_value::<u128>(self.env(), v);
    }

    pub fn observe_mint(&mut self) -> Result<()> {
        let height = self.height().to_le_bytes().to_vec();
        let mut pointer = Self::seen_pointer(&height);
        if pointer.get(self.env()).len() == 0 {
            pointer.set_value::<u32>(self.env(), 1);
            Ok(())
        } else {
            Err(anyhow!(format!(
                "already minted for block {}",
                hex::encode(&height)
            )))
        }
    }

    pub fn observe_upgraded_mint(&mut self, diesel_fee: u128) -> Result<()> {
        let height = self.height().to_le_bytes().to_vec();
        let mut pointer = Self::upgraded_seen_pointer(&height);
        if pointer.get(self.env()).len() == 0 {
            pointer.set_value::<u32>(self.env(), 1);
            if Self::claimable_fees_pointer().get(self.env()).len() == 0 {
                self.set_claimable_fees(0);
            }
            self.increase_claimable_fees(diesel_fee)?;
            self.increase_total_supply(diesel_fee)?;
        }
        Ok(())
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&mut self) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        self.observe_mint()?;
        let value = self.current_block_reward();
        let mut total_supply_pointer = Self::total_supply_pointer();
        let total_supply = total_supply_pointer.get_value::<u128>(self.env());
        if total_supply >= self.max_supply() {
            return Err(anyhow!("total supply has been reached"));
        }
        total_supply_pointer.set_value::<u128>(self.env(), total_supply + value);
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value,
        })
    }

    /// Check if a transaction hash has been used for minting
    pub fn has_tx_hash(&mut self, txid: &Txid) -> bool {
        StoragePointer::from_keyword("/tx-hashes/")
            .select(&txid.as_byte_array().to_vec())
            .get_value::<u8>(self.env())
            == 1
    }

    /// Add a transaction hash to the used set
    pub fn add_tx_hash(&mut self, txid: &Txid) -> Result<()> {
        StoragePointer::from_keyword("/tx-hashes/")
            .select(&txid.as_byte_array().to_vec())
            .set_value::<u8>(self.env(), 0x01);
        Ok(())
    }

    fn enforce_one_mint_per_tx(&mut self) -> Result<()> {
        // Get transaction ID
        let txid = self.transaction_id()?;

        // Enforce one mint per transaction
        if self.has_tx_hash(&txid) {
            return Err(anyhow!("Transaction already used for minting"));
        }

        // Record transaction hash
        self.add_tx_hash(&txid)?;
        Ok(())
    }

    fn enforce_no_upgraded_mints_with_legacy_mints(&mut self) -> Result<()> {
        let legacy_mint_pointer = Self::seen_pointer(&self.height().to_le_bytes().to_vec());
        if legacy_mint_pointer.get(self.env()).len() == 0 {
            Ok(())
        } else {
            Err(anyhow!(format!(
                "upgraded mint in the same block as legacy mint",
            )))
        }
    }

    // Helper method that creates a mint transfer
    pub fn create_upgraded_mint_transfer(&mut self) -> Result<AlkaneTransfer> {
        let context = self.context()?;

        self.enforce_one_mint_per_tx()?;
        self.enforce_no_upgraded_mints_with_legacy_mints()?;

        let total_mints = self.number_diesel_mints()?;
        let total_miner_fee = self.total_miner_fee()?;
        let block_reward = self.current_block_reward();
        let total_tx_fee = if total_miner_fee > block_reward {
            total_miner_fee - block_reward
        } else {
            0
        };
        let diesel_fee = std::cmp::min(block_reward / 2, total_tx_fee); // fee is capped at 50% of the block reward
        let value_per_mint = (block_reward - diesel_fee) / total_mints;
        self.observe_upgraded_mint(diesel_fee)?;

        if self.total_supply() >= self.max_supply() {
            return Err(anyhow!("total supply has been reached"));
        }
        self.increase_total_supply(value_per_mint)?;
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value: value_per_mint,
        })
    }

    fn observe_upgrade_initialization(&mut self) -> Result<()> {
        let context = self.context()?;
        let premine = self.premine()?;
        if !context
            .incoming_alkanes
            .0
            .iter()
            .any(|i| (i.id == context.myself && i.value == premine))
        {
            return Err(anyhow!("Premine is not spent into the upgrade"));
        }
        let mut pointer = StoragePointer::from_keyword("/upgrade_initialized");
        if pointer.get(self.env()).len() == 0 {
            pointer.set_value::<u8>(self.env(), 0x01);
            Ok(())
        } else {
            Err(anyhow!("already upgraded diesel"))
        }
    }

    fn initialize(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.observe_mint()?;
        self.observe_initialization()?;
        let premine = self.premine()?;
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: premine,
        });
        self.set_total_supply(premine);

        Ok(response)
    }

    fn upgrade(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.observe_upgrade_initialization()?;
        response.alkanes.0.push(self.deploy_auth_token(5)?); // hardcode 5 auth tokens

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&mut self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        if StoragePointer::from_keyword("/upgrade_initialized")
            .get(self.env())
            .len()
            == 0
        {
            response.alkanes.0.push(self.create_mint_transfer()?);
        } else {
            response
                .alkanes
                .0
                .push(self.create_upgraded_mint_transfer()?);
        }

        Ok(response)
    }

    fn collect_fees(&mut self) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let fees = self.claimable_fees();
        response.alkanes.pay(AlkaneTransfer {
            id: context.myself,
            value: fees,
        });
        self.set_claimable_fees(0);
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
        let supply = self.total_supply();
        response.data = (&supply.to_le_bytes()).to_vec();

        Ok(response)
    }
}

impl AuthenticatedResponder for GenesisAlkane {}
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
