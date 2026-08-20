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
    SetMintGate {
        gate_block: u128,
        gate_tx: u128,
    },

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

/// Dead-man's switch: a governance-set mint gate auto-expires to the default
/// (off) if not re-affirmed within this many blocks (~1 year at 6 blk/hr).
const GATE_TTL: u64 = 52_560;

/// Timelock on gate changes: a newly set gate only takes effect after this many
/// blocks (~1 week at 6 blk/hr). Every change is therefore publicly visible (via
/// `get_mint_gate`) BEFORE any emission flows through it, giving the community
/// real time to notice, discuss and react. Turning the gate OFF (0:0) is exempt
/// and immediate: returning to the safe default must never wait out a delay.
const GATE_DELAY: u64 = 1_008;

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

        // Mint gate: when governance has set a live gate, forward the freshly
        // minted DIESEL to it as incoming_alkanes and pass the calldata FOLLOWING
        // the mint opcode (77) through as the gate's cellpack inputs. The gate
        // (e.g. a share-token contract) receives the mint WITHOUT subcalling
        // DIESEL back, while the user still calls the real [2,0] contract. The
        // gate only ever sees the minted DIESEL; whatever it returns, plus any
        // stray tokens attached to the mint, are handed back to the caller.
        if let Some(gate) = self.mint_gate() {
            let forward_calldata: Vec<u128> =
                context.inputs.iter().skip(1).cloned().collect();
            if !forward_calldata.is_empty() {
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

        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.alkanes.0.push(transfer);
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

    // ---- Mint gate: one governance-set forward target ------------------------
    //
    // Two slots. `/mint-gate` holds the latest governance-set target and its
    // set height; it only GOVERNS once `GATE_DELAY` blocks have passed.
    // `/mint-gate-prev` holds the last target that actually activated, and
    // governs during a newer target's pending window. Only an ACTIVATED slot is
    // ever demoted to prev — re-setting within the window cannot promote a
    // never-activated target past its own delay.

    fn mint_gate_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate")
    }

    fn mint_gate_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate-height")
    }

    fn mint_gate_prev_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate-prev")
    }

    fn mint_gate_prev_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate-prev-height")
    }

    fn read_gate_slot(ptr: StoragePointer) -> AlkaneId {
        let raw = ptr.get();
        if raw.len() == 0 {
            return AlkaneId::new(0, 0);
        }
        AlkaneId::try_from(raw.as_ref().clone()).unwrap_or(AlkaneId::new(0, 0))
    }

    /// The slot that governs mints at the current height: the latest set target
    /// once its `GATE_DELAY` timelock has elapsed, else the previously
    /// activated one.
    fn governing_slot(&self) -> (AlkaneId, u64) {
        let cur_h = self.mint_gate_height_pointer().get_value::<u64>();
        if self.height() >= cur_h.saturating_add(GATE_DELAY) {
            (Self::read_gate_slot(self.mint_gate_pointer()), cur_h)
        } else {
            (
                Self::read_gate_slot(self.mint_gate_prev_pointer()),
                self.mint_gate_prev_height_pointer().get_value::<u64>(),
            )
        }
    }

    /// The live mint-gate target, or None. None when unset, cleared (0:0),
    /// still inside its timelock, or EXPIRED: the gate reverts to the default
    /// (off) if governance has not re-affirmed it within `GATE_TTL` blocks — a
    /// dead-man's switch so a gate can never outlive governance's attention.
    fn mint_gate(&self) -> Option<AlkaneId> {
        let (gate, set_height) = self.governing_slot();
        if gate.block == 0 && gate.tx == 0 {
            return None;
        }
        if self.height() > set_height.saturating_add(GATE_TTL) {
            return None;
        }
        Some(gate)
    }

    /// Owner-only switch (auth token in incoming_alkanes). Point DIESEL's mint
    /// gate at `gate_block:gate_tx` — the change is TIMELOCKED and only takes
    /// effect `GATE_DELAY` blocks later, publicly visible in `get_mint_gate`
    /// the whole time. `0:0` turns the gate off IMMEDIATELY (both slots
    /// cleared): the safe default is never delayed. Every set stamps the
    /// height, resetting the dead-man's-switch TTL.
    fn set_mint_gate(&self, gate_block: u128, gate_tx: u128) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        let gate = AlkaneId::new(gate_block, gate_tx);
        let now = self.height();

        if gate.block == 0 && gate.tx == 0 {
            self.mint_gate_pointer().set(Arc::new(Vec::new()));
            self.mint_gate_prev_pointer().set(Arc::new(Vec::new()));
            self.mint_gate_height_pointer().set_value::<u64>(now);
            self.mint_gate_prev_height_pointer().set_value::<u64>(now);
            return Ok(CallResponse::forward(&context.incoming_alkanes));
        }

        // Demote the current slot to fallback ONLY if it has activated.
        let cur_h = self.mint_gate_height_pointer().get_value::<u64>();
        if now >= cur_h.saturating_add(GATE_DELAY) {
            self.mint_gate_prev_pointer()
                .set(self.mint_gate_pointer().get());
            self.mint_gate_prev_height_pointer().set_value::<u64>(cur_h);
        }

        self.mint_gate_pointer()
            .set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(gate)));
        self.mint_gate_height_pointer().set_value::<u64>(now);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    /// Read-only audit view of the mint gate. Shows BOTH the governing gate and
    /// any pending (timelocked) change, so every switch is publicly visible for
    /// the full `GATE_DELAY` window before emission flows through it. Fixed LE
    /// layout:
    ///   [0..16)  governing gate block u128 (0:0 = none)
    ///   [16..32) governing gate tx u128
    ///   [32..40) governing set_height u64 (0 if none)
    ///   [40..48) governing expiry u64 = set_height + GATE_TTL (0 if none)
    ///   [48..64) pending gate block u128 (0:0 = nothing pending)
    ///   [64..80) pending gate tx u128
    ///   [80..88) pending activation height u64 = set_height + GATE_DELAY
    ///   [88..96) current height u64
    ///   [96]     live u8 (1 = the governing gate is in force right now)
    fn get_mint_gate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let now = self.height();

        let (gov, gov_h) = self.governing_slot();
        let live: u8 = if self.mint_gate().is_some() { 1 } else { 0 };
        let (gov_set, gov_exp) = if gov.block == 0 && gov.tx == 0 {
            (0u64, 0u64)
        } else {
            (gov_h, gov_h.saturating_add(GATE_TTL))
        };

        let cur_h = self.mint_gate_height_pointer().get_value::<u64>();
        let (pend, pend_act) = if now >= cur_h.saturating_add(GATE_DELAY) {
            (AlkaneId::new(0, 0), 0u64)
        } else {
            (
                Self::read_gate_slot(self.mint_gate_pointer()),
                cur_h.saturating_add(GATE_DELAY),
            )
        };

        let mut data = Vec::with_capacity(97);
        data.extend_from_slice(&gov.block.to_le_bytes());
        data.extend_from_slice(&gov.tx.to_le_bytes());
        data.extend_from_slice(&gov_set.to_le_bytes());
        data.extend_from_slice(&gov_exp.to_le_bytes());
        data.extend_from_slice(&pend.block.to_le_bytes());
        data.extend_from_slice(&pend.tx.to_le_bytes());
        data.extend_from_slice(&pend_act.to_le_bytes());
        data.extend_from_slice(&now.to_le_bytes());
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
