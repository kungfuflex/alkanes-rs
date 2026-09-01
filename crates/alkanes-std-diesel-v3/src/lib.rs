use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::{auth::AuthenticatedResponder, declare_alkane};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::utils::overflow_error;
use alkanes_support::{
    context::Context,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use bitcoin::{Block, Txid};
use hex;
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::io::Cursor;
use std::sync::Arc;
pub mod chain;
use crate::chain::{ChainConfiguration, CONTEXT_HANDLE};

#[derive(Default)]
pub struct GenesisAlkane(());

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

    #[opcode(79)]
    Burn { amount: u128 },

    #[opcode(80)]
    SetMintGate { gate_block: u128, gate_tx: u128 },

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    #[opcode(102)]
    #[returns(Vec<u8>)]
    GetMintGate,
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
    fn premine(&self) -> Result<u128> {
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
    fn premine(&self) -> Result<u128> {
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

    pub fn seen_pointer(&self, hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/seen/").select(&hash)
    }

    pub fn upgraded_seen_pointer(&self, hash: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/upgraded_seen/").select(&hash)
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

    pub fn decrease_total_supply(&self, v: u128) -> Result<()> {
        self.set_total_supply(overflow_error(self.total_supply().checked_sub(v))?);
        Ok(())
    }

    pub fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }

    pub fn observe_mint(&self) -> Result<()> {
        let height = self.height().to_le_bytes().to_vec();
        let mut pointer = self.seen_pointer(&height);
        if pointer.get().len() == 0 {
            pointer.set_value::<u32>(1);
            Ok(())
        } else {
            Err(anyhow!(format!(
                "already minted for block {}",
                hex::encode(&height)
            )))
        }
    }

    pub fn observe_upgraded_mint(&self, diesel_fee: u128) -> Result<()> {
        let height = self.height().to_le_bytes().to_vec();
        let mut pointer = self.upgraded_seen_pointer(&height);
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

    /// Check if a transaction hash has been used for minting
    pub fn has_tx_hash(&self, txid: &Txid) -> bool {
        StoragePointer::from_keyword("/tx-hashes/")
            .select(&txid.as_byte_array().to_vec())
            .get_value::<u8>()
            == 1
    }

    /// Add a transaction hash to the used set
    pub fn add_tx_hash(&self, txid: &Txid) -> Result<()> {
        StoragePointer::from_keyword("/tx-hashes/")
            .select(&txid.as_byte_array().to_vec())
            .set_value::<u8>(0x01);
        Ok(())
    }

    fn enforce_one_mint_per_tx(&self) -> Result<()> {
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

    fn enforce_no_upgraded_mints_with_legacy_mints(&self) -> Result<()> {
        let legacy_mint_pointer = self.seen_pointer(&self.height().to_le_bytes().to_vec());
        if legacy_mint_pointer.get().len() == 0 {
            Ok(())
        } else {
            Err(anyhow!(format!(
                "upgraded mint in the same block as legacy mint",
            )))
        }
    }

    // Helper method that creates a mint transfer
    pub fn create_upgraded_mint_transfer(&self) -> Result<AlkaneTransfer> {
        let context = self.context()?;

        if context.caller != AlkaneId::new(0, 0) {
            return Err(anyhow!(
                "Diesel mint must be called from EOA (first call in a protostone)"
            ));
        }
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

    fn observe_upgrade_initialization(&self) -> Result<()> {
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
        if pointer.get().len() == 0 {
            pointer.set_value::<u8>(0x01);
            Ok(())
        } else {
            Err(anyhow!("already upgraded diesel"))
        }
    }

    fn initialize(&self) -> Result<CallResponse> {
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

    fn upgrade(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.observe_upgrade_initialization()?;
        response.alkanes.0.push(self.deploy_auth_token(5)?); // hardcode 5 auth tokens

        Ok(response)
    }

    // Method that matches the MessageDispatch enum
    fn mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let transfer = if StoragePointer::from_keyword("/upgrade_initialized")
            .get()
            .len()
            == 0
        {
            self.create_mint_transfer()?
        } else {
            self.create_upgraded_mint_transfer()?
        };

        // Mint gate: when governance has set a gate, forward the freshly minted
        // DIESEL to it as incoming_alkanes and pass the calldata FOLLOWING the
        // mint opcode (77) through as the gate's cellpack inputs. The gate (e.g.
        // a share-token contract) receives the mint WITHOUT subcalling DIESEL
        // back, while the user still calls the real [2,0] contract. The gate
        // only ever sees the minted DIESEL; whatever it returns, plus any stray
        // tokens attached to the mint, are handed back to the caller.
        if let Some(gate) = self.mint_gate() {
            let forward_calldata: Vec<u128> = context.inputs.iter().skip(1).cloned().collect();
            // Forward ONLY when the gate opcode (the first word after 77) is
            // non-zero. Calldata is zero-padded on the wire, so a bare mint
            // ([77]) arrives as [77, 0, 0, ...]; forwarding that would call the
            // gate with opcode 0 (Initialize) and revert every ordinary mint.
            // A zero lead word is never a real dispatch opcode, so this is also
            // the correct guard.
            if forward_calldata.first().is_some_and(|&op| op != 0) {
                let parcel = AlkaneTransferParcel(vec![transfer]);
                let cellpack = Cellpack {
                    target: gate,
                    inputs: forward_calldata,
                };
                let mut response = self.call(&cellpack, &parcel, self.fuel())?;
                response
                    .alkanes
                    .0
                    .extend(context.incoming_alkanes.0.iter().cloned());
                return Ok(response);
            }
        }

        // Default (no gate, or a bare mint with no gate calldata): the mint is
        // NOT paid to the caller. It accrues to the contract's claimable-fees
        // balance, which only the DSIGIL auth-token holder can withdraw via
        // `collect_fees` (opcode 78, `only_owner`). This is the v3 default:
        // emission collects in the contract until governance points it at a
        // gate.
        self.increase_claimable_fees(transfer.value)?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
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

    // ---- Mint gate: one governance-set forward target ------------------------
    //
    // A single slot holding the current target. Changes take effect
    // immediately: there is no timelock and no expiry. `0:0` (or unset) means
    // no gate, in which case the mint accrues to claimable fees for the owner.

    fn mint_gate_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate")
    }

    /// The current mint-gate target, or None when unset or explicitly cleared
    /// to `0:0`.
    fn mint_gate(&self) -> Option<AlkaneId> {
        let raw = self.mint_gate_pointer().get();
        if raw.len() == 0 {
            return None;
        }
        let gate = AlkaneId::try_from(raw.as_ref().clone()).unwrap_or(AlkaneId::new(0, 0));
        if gate.block == 0 && gate.tx == 0 {
            return None;
        }
        Some(gate)
    }

    /// Owner-only switch (auth token in incoming_alkanes). Point DIESEL's mint
    /// gate at `gate_block:gate_tx`, effective immediately. `0:0` clears it,
    /// returning to the default where emission accrues to claimable fees.
    fn set_mint_gate(&self, gate_block: u128, gate_tx: u128) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        let gate = AlkaneId::new(gate_block, gate_tx);

        if gate.block == 0 && gate.tx == 0 {
            self.mint_gate_pointer().set(Arc::new(Vec::new()));
        } else {
            self.mint_gate_pointer()
                .set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(gate)));
        }
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    /// Read-only audit view of the mint gate. Fixed little-endian layout:
    ///   [0..16)  gate block u128 (0:0 = no gate)
    ///   [16..32) gate tx u128
    ///   [32..40) current height u64
    ///   [40]     live u8 (1 = a gate is set)
    fn get_mint_gate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let gate = self.mint_gate().unwrap_or(AlkaneId::new(0, 0));
        let live: u8 = if self.mint_gate().is_some() { 1 } else { 0 };

        let mut data = Vec::with_capacity(41);
        data.extend_from_slice(&gate.block.to_le_bytes());
        data.extend_from_slice(&gate.tx.to_le_bytes());
        data.extend_from_slice(&self.height().to_le_bytes());
        data.push(live);
        response.data = data;
        Ok(response)
    }

    fn burn(&self, amount: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::default();
        for transfer in context.incoming_alkanes.0 {
            if transfer.id == context.myself {
                if amount > transfer.value {
                    return Err(anyhow!("attempting to burn more than input amount"));
                }
                response.alkanes.pay(AlkaneTransfer {
                    id: context.myself,
                    value: transfer.value - amount,
                });
                self.decrease_total_supply(amount)?;
            } else {
                response.alkanes.pay(transfer);
            }
        }

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
