use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::declare_alkane;
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

/// Opcode DIESEL invokes on the mint gate. FIXED by DIESEL and never taken
/// from the minter's calldata: if the minter could pick the opcode, `[77, 102]`
/// (or any view that does `CallResponse::forward(incoming)`) would hand the
/// whole mint straight back, bypassing the split. 1 = SUBMINER-GATE
/// `mint-split`.
pub const MINT_GATE_OPCODE: u128 = 1;

/// The mint gate in effect when governance has never set one (`/mint-gate`
/// storage empty): SUBMINER-GATE, deployed by create-at-reserved-slot
/// `[3, 1001, 0, 2,0, 2,96782, 4,866, 192]` and so living at `4:1001`.
///
/// 🔴 RELEASE GATE: this id MUST be checked against the actually deployed
/// SUBMINER-GATE (id AND codehash, and its initialize args diesel=2:0,
/// treasury=2:96782, sigil=4:866) before the release that activates DIESEL v3
/// is tagged. Reserved slots are first-come: whoever deploys at 4:1001 first
/// receives every DIESEL mint from activation on. If nothing is deployed there
/// at activation, every mint reverts (the runtime traps on a missing binary).
/// Same id on every network so the regtest test-suite exercises the mainnet
/// default (regtest only activates v3 at 2_000_000).
pub const DEFAULT_MINT_GATE: AlkaneId = AlkaneId { block: 4, tx: 1001 };

/// Fuel DIESEL keeps back from the mint gate so that, if the gate reverts
/// (or burns its whole allotment), DIESEL can still route the mint to
/// claimable fees instead of reverting the protostone. The gate gets
/// `fuel() - GATE_FALLBACK_RESERVE`. Requires the post-audit runtime, which
/// honors the extcall fuel argument as a cap and leaves the caller its unallotted
/// fuel when the callee reverts. Sized from `tests::mint_gate`: the fallback
/// (extcall charge, revert read-back, claimable-fees write, response) measures
/// ~106k against a gate that reverts or spins until out of fuel, so this keeps
/// a >4x margin. The mainnet SUBMINER-GATE + treasury deposit measures ~330k,
/// against ~2.65M allotted on a minimum-fuel (3.5M) mint tx.
pub const GATE_FALLBACK_RESERVE: u64 = 500_000;

/// First height at which `2:0` runs this binary. MUST equal the indexer's
/// `genesis::DIESEL_V3_BLOCK_HEIGHT` for the network this artifact is built
/// for (the indexer only ships the mainnet and regtest builds of v3). Blocks
/// from here on are covered by the emission ledger: none of their reward is
/// lost (see `settle_unissued`).
#[cfg(feature = "mainnet")]
pub const V3_ACTIVATION_HEIGHT: u64 = 972_000;
#[cfg(not(feature = "mainnet"))]
pub const V3_ACTIVATION_HEIGHT: u64 = 2_000_000;

/// cFIRE-SIGIL: the bearer capability that governs the mint gate (opcode 80).
/// Possession-based, mirroring SUBMINER-GATE's own `require_auth`: >= 1 unit in
/// incoming_alkanes, and the sigil's `authenticate` (opcode 1) must return
/// `[0x01]`. DIESEL's own DSIGIL auth token does NOT govern the gate.
pub const CFIRE_SIGIL: AlkaneId = AlkaneId { block: 4, tx: 866 };

/// The sigil's `authenticate` opcode.
const SIGIL_AUTHENTICATE_OPCODE: u128 = 1;

/// Mint-gate source byte reported by the view (opcode 102, byte 41).
const GATE_SOURCE_DEFAULT: u8 = 0;
const GATE_SOURCE_SET: u8 = 1;
const GATE_SOURCE_CLEARED: u8 = 2;

/// The largest amount `<= value` that can be minted on top of `total_supply`
/// without the result exceeding `max_supply`. Never overflows: headroom is a
/// saturating subtraction, and `total_supply + result <= max_supply`.
pub fn clamp_to_cap(value: u128, total_supply: u128, max_supply: u128) -> u128 {
    std::cmp::min(value, max_supply.saturating_sub(total_supply))
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
            self.settle_unissued()?;
            // Fee accrual mints new supply too, so it is bounded by the cap:
            // clamp it to the remaining headroom (Halborn: "Supply Cap Check
            // Allows the Final DIESEL Mint to Exceed the Limit").
            let diesel_fee = clamp_to_cap(diesel_fee, self.total_supply(), self.max_supply());
            self.increase_claimable_fees(diesel_fee)?;
            self.increase_total_supply(diesel_fee)?;
            self.record_issued(diesel_fee)?;
        }
        Ok(())
    }

    // ---- Emission ledger: no block's reward is lost --------------------------
    //
    // From `V3_ACTIVATION_HEIGHT` on, every block issues exactly its
    // `block_reward`. Minters receive what the block's mints pay out; whatever
    // they do not receive goes to claimable fees (cFIRE-SIGIL, opcode 78):
    //   - a block with NO mints: its whole reward;
    //   - counted mints that fail (non-EOA caller, legacy mint in the block,
    //     supply cap, a gate that over-returns): their share;
    //   - anything else short of the reward.
    // There is no end-of-block hook, so the shortfall is settled lazily: the
    // first successful mint of a block settles everything owed since the ledger
    // was last opened, then opens the ledger for its own block. A mint that
    // reverts rolls back its own ledger writes, so only real issuance is
    // recorded. Settlement is bounded by the supply cap like any other accrual.
    //
    // `/emission/height`: the block whose ledger is open (empty = none yet).
    // `/emission/issued`: what that block has issued so far.

    fn emission_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/emission/height")
    }

    fn emission_issued_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/emission/issued")
    }

    /// Sum of `block_reward(k)` for `from <= k < to`, one multiplication per
    /// halving era so a long mint-free stretch costs O(eras), not O(blocks).
    fn rewards_between(&self, from: u64, to: u64) -> Result<u128> {
        let mut total: u128 = 0;
        let mut k = from;
        while k < to {
            let era_end = (k / 210_000 + 1) * 210_000;
            let seg_end = std::cmp::min(to, era_end);
            let span = (seg_end - k) as u128;
            total = overflow_error(
                total.checked_add(overflow_error(span.checked_mul(self.block_reward(k)))?),
            )?;
            k = seg_end;
        }
        Ok(total)
    }

    /// First successful mint of the block at `height`: issue into claimable
    /// fees what earlier blocks still owe, then open the ledger for `height`.
    fn settle_unissued(&self) -> Result<()> {
        let height = self.height();
        let mut height_ptr = self.emission_height_pointer();
        let mut issued_ptr = self.emission_issued_pointer();
        let owed = if height_ptr.get().len() == 0 {
            // First ledger ever: every block since activation that had no
            // successful mint is owed in full.
            self.rewards_between(V3_ACTIVATION_HEIGHT, height)?
        } else {
            let open = height_ptr.get_value::<u64>();
            if open >= height {
                // Cannot happen (the per-height seen marker gates this call),
                // but never settle a block twice.
                return Ok(());
            }
            let reward = self.block_reward(open);
            let shortfall = reward.saturating_sub(issued_ptr.get_value::<u128>());
            overflow_error(shortfall.checked_add(self.rewards_between(open + 1, height)?))?
        };
        let owed = clamp_to_cap(owed, self.total_supply(), self.max_supply());
        if owed > 0 {
            self.increase_claimable_fees(owed)?;
            self.increase_total_supply(owed)?;
        }
        height_ptr.set_value::<u64>(height);
        issued_ptr.set_value::<u128>(0);
        Ok(())
    }

    /// Count `v` of new supply against the open (current) block's reward.
    fn record_issued(&self, v: u128) -> Result<()> {
        let mut issued_ptr = self.emission_issued_pointer();
        issued_ptr.set_value::<u128>(overflow_error(issued_ptr.get_value::<u128>().checked_add(v))?);
        Ok(())
    }

    /// Commit a mint of up to `value` against the supply cap and return the
    /// amount actually minted. The FULL post-mint supply is checked, not just
    /// the pre-mint supply: the final mint is clamped to the remaining
    /// headroom so total supply lands exactly on `max_supply` and can never
    /// exceed it (nor overflow u128 on the u128::MAX regtest cap). A mint with
    /// zero headroom (supply at — or, for pre-existing state, above — the cap)
    /// is rejected, which reverts every tentative write of the call (seen
    /// markers, tx-hash marker, fee accrual).
    fn commit_capped_mint(&self, value: u128) -> Result<u128> {
        let total_supply = self.total_supply();
        let minted = clamp_to_cap(value, total_supply, self.max_supply());
        if minted == 0 {
            return Err(anyhow!("total supply has been reached"));
        }
        self.set_total_supply(overflow_error(total_supply.checked_add(minted))?);
        Ok(minted)
    }

    // Helper method that creates a mint transfer
    pub fn create_mint_transfer(&self) -> Result<AlkaneTransfer> {
        let context = self.context()?;
        self.observe_mint()?;
        let value = self.commit_capped_mint(self.current_block_reward())?;
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
        // The integer-division remainder goes to claimable fees with the
        // block's fee accrual (first mint only), so when every counted mint
        // executes the block emits exactly `block_reward`, never more.
        let diesel_fee = diesel_fee + (block_reward - diesel_fee) % total_mints;
        // The minter's share is committed against the cap FIRST and the fee
        // accrual (first mint of the block only) gets whatever headroom is
        // left. The reverse order lets the fee swallow the last headroom and
        // then reject the mint, which reverts the fee with it, so supply would
        // stall below the cap forever.
        let value_per_mint = self.commit_capped_mint(value_per_mint)?;
        // Settles earlier blocks and opens this block's ledger on the first
        // successful mint, so this mint is recorded against THIS block.
        self.observe_upgraded_mint(diesel_fee)?;
        self.record_issued(value_per_mint)?;
        Ok(AlkaneTransfer {
            id: context.myself.clone(),
            value: value_per_mint,
        })
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

    /// Retired in v3. The premine upgrade ran long before activation on
    /// mainnet; it was also the only path that writes DIESEL's `/auth` slot
    /// (the DSIGIL auth token). With it retired and `collect_fees` gated on the
    /// cFIRE-SIGIL, v3 never reads or writes `/auth`: DSIGIL is frozen out from
    /// `V3_ACTIVATION_HEIGHT` on.
    fn upgrade(&self) -> Result<CallResponse> {
        Err(anyhow!("DIESEL v3: upgrade is retired"))
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

        // Mint gate: EVERY mint (bare `[77]` included) is forwarded to the gate
        // as incoming_alkanes, calling the gate with the FIXED opcode
        // `MINT_GATE_OPCODE` and no other calldata. Whatever follows 77 in the
        // minter's calldata is ignored, so the minter cannot choose which gate
        // entry point receives the mint. The gate decides the split and
        // returns the minter's share; stray tokens attached to the mint are
        // handed back to the caller and never reach the gate.
        if let Some(gate) = self.mint_gate() {
            let parcel = AlkaneTransferParcel(vec![transfer.clone()]);
            let cellpack = Cellpack {
                target: gate,
                inputs: vec![MINT_GATE_OPCODE],
            };
            // Cap the gate's fuel so DIESEL keeps GATE_FALLBACK_RESERVE for
            // the fallback below. Never pass 0: the runtime reads 0 as "all".
            let gate_fuel = self.fuel().saturating_sub(GATE_FALLBACK_RESERVE).max(1);
            match self.call(&cellpack, &parcel, gate_fuel) {
                Ok(mut response) => {
                    // The gate may only pay out of the mint it was handed. A
                    // gate returning more DIESEL than was minted (it can only
                    // do so out of DIESEL it already held) is a broken or
                    // hostile gate: error out so the WHOLE call reverts
                    // atomically. Falling back to fees here would not work —
                    // the gate's call already succeeded, so its state and the
                    // excess transfer are committed in this frame.
                    let mut returned: u128 = 0;
                    for t in response.alkanes.0.iter() {
                        if t.id == context.myself {
                            returned = overflow_error(returned.checked_add(t.value))?;
                        }
                    }
                    if returned > transfer.value {
                        return Err(anyhow!(
                            "mint gate returned {} DIESEL for a mint of {}",
                            returned,
                            transfer.value
                        ));
                    }
                    response.data = Vec::new();
                    response
                        .alkanes
                        .0
                        .extend(context.incoming_alkanes.0.iter().cloned());
                    return Ok(response);
                }
                Err(_) => {
                    // Gate reverted (or ran out of its capped fuel). The
                    // runtime rolls back the child frame, including the
                    // transfer of the minted DIESEL into it, and leaves DIESEL
                    // its reserved fuel, so fall through to the accrual
                    // default: the mint goes to claimable fees (cFIRE-SIGIL) and the
                    // protostone still succeeds. Minting must never brick
                    // because the gate or its treasury is broken.
                }
            }
        }

        // No gate (explicitly cleared), or the gate reverted: the mint is NOT
        // paid to the caller. It accrues to the contract's claimable-fees
        // balance, which only the cFIRE-SIGIL bearer can withdraw via
        // `collect_fees` (opcode 78).
        self.increase_claimable_fees(transfer.value)?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    /// Pay out everything claimable. Gated on the cFIRE-SIGIL (4:866), the
    /// same bearer check as `set_mint_gate`. DIESEL's legacy DSIGIL auth token
    /// (`/auth`, `only_owner`) no longer authorizes anything in v3.
    fn collect_fees(&self) -> Result<CallResponse> {
        self.require_cfire_sigil()?;
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
    // `/mint-gate` storage:
    //   empty            -> DEFAULT_MINT_GATE (no governance tx needed);
    //   32 zero bytes    -> explicitly cleared by governance: no gate;
    //   AlkaneId bytes   -> that gate.
    // Changes take effect immediately: there is no timelock and no expiry.

    fn mint_gate_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/mint-gate")
    }

    /// The effective gate and where it came from.
    fn mint_gate_with_source(&self) -> (Option<AlkaneId>, u8) {
        let raw = self.mint_gate_pointer().get();
        if raw.len() == 0 {
            return (Some(DEFAULT_MINT_GATE), GATE_SOURCE_DEFAULT);
        }
        let gate = AlkaneId::try_from(raw.as_ref().clone()).unwrap_or(AlkaneId::new(0, 0));
        if gate.block == 0 && gate.tx == 0 {
            return (None, GATE_SOURCE_CLEARED);
        }
        (Some(gate), GATE_SOURCE_SET)
    }

    /// The effective mint-gate target, or None when governance cleared it.
    fn mint_gate(&self) -> Option<AlkaneId> {
        self.mint_gate_with_source().0
    }

    /// Capability-by-possession check against the cFIRE-SIGIL, mirroring
    /// SUBMINER-GATE's `require_auth`: >= 1 unit in incoming, then the sigil's
    /// authenticate op must return `[0x01]`. Non-consuming: the caller forwards
    /// incoming (sigil included) back.
    fn require_cfire_sigil(&self) -> Result<()> {
        let context = self.context()?;
        if !context
            .incoming_alkanes
            .0
            .iter()
            .any(|t| t.id == CFIRE_SIGIL && t.value > 0)
        {
            return Err(anyhow!("cFIRE-SIGIL not present in incoming alkanes"));
        }
        let response = self.call(
            &Cellpack {
                target: CFIRE_SIGIL,
                inputs: vec![SIGIL_AUTHENTICATE_OPCODE],
            },
            &AlkaneTransferParcel(vec![AlkaneTransfer {
                id: CFIRE_SIGIL,
                value: 1,
            }]),
            self.fuel(),
        )?;
        if response.data != vec![0x01] {
            return Err(anyhow!("cFIRE-SIGIL authentication failed"));
        }
        Ok(())
    }

    /// cFIRE-SIGIL-gated switch. Point DIESEL's mint gate at
    /// `gate_block:gate_tx`, effective immediately. `0:0` clears it (persisted
    /// as an explicit "no gate", distinct from the unset default), returning to
    /// the default where emission accrues to claimable fees.
    fn set_mint_gate(&self, gate_block: u128, gate_tx: u128) -> Result<CallResponse> {
        self.require_cfire_sigil()?;
        let context = self.context()?;
        let gate = AlkaneId::new(gate_block, gate_tx);
        // 0:0 serializes to 32 zero bytes: the "cleared" sentinel.
        self.mint_gate_pointer()
            .set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(gate)));
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    /// Read-only audit view of the mint gate. Fixed little-endian layout:
    ///   [0..16)  effective gate block u128 (0:0 = no gate)
    ///   [16..32) effective gate tx u128
    ///   [32..40) current height u64
    ///   [40]     live u8 (1 = a gate is in effect)
    ///   [41]     source u8 (0 = compile-time default, 1 = set by governance,
    ///            2 = cleared by governance)
    fn get_mint_gate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let (gate, source) = self.mint_gate_with_source();
        let live: u8 = if gate.is_some() { 1 } else { 0 };
        let gate = gate.unwrap_or(AlkaneId::new(0, 0));

        let mut data = Vec::with_capacity(42);
        data.extend_from_slice(&gate.block.to_le_bytes());
        data.extend_from_slice(&gate.tx.to_le_bytes());
        data.extend_from_slice(&self.height().to_le_bytes());
        data.push(live);
        data.push(source);
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

impl AlkaneResponder for GenesisAlkane {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for GenesisAlkane {
        type Message = GenesisAlkaneMessage;
    }
}

#[cfg(test)]
mod cap_tests {
    use super::clamp_to_cap;
    const CAP: u128 = 156_250_000_000_000; // mainnet max_supply
    const REWARD: u128 = 312_500_000;

    #[test]
    fn full_mint_below_cap() {
        assert_eq!(clamp_to_cap(REWARD, CAP - REWARD - 1, CAP), REWARD);
    }

    #[test]
    fn final_mint_exactly_reaches_cap() {
        assert_eq!(clamp_to_cap(REWARD, CAP - REWARD, CAP), REWARD);
    }

    #[test]
    fn final_mint_clamped_to_headroom() {
        // Halborn PoC state: post-fee supply = cap - 1.
        assert_eq!(clamp_to_cap(REWARD, CAP - 1, CAP), 1);
        assert_eq!(clamp_to_cap(REWARD, 156_249_843_749_999, CAP), 156_250_001);
    }

    #[test]
    fn at_or_above_cap_mints_nothing() {
        assert_eq!(clamp_to_cap(REWARD, CAP, CAP), 0);
        assert_eq!(clamp_to_cap(REWARD, CAP + 5, CAP), 0);
    }

    #[test]
    fn no_overflow_on_u128_max_cap() {
        assert_eq!(clamp_to_cap(REWARD, u128::MAX - 1, u128::MAX), 1);
        assert_eq!(clamp_to_cap(u128::MAX, u128::MAX, u128::MAX), 0);
        assert_eq!(clamp_to_cap(u128::MAX, 1, u128::MAX), u128::MAX - 1);
    }
}
