//! The 12:0 alkane shell.
//!
//! Deliberately thin: every decision lives in [`crate::abi`], which is pure and
//! natively testable. This file only moves bytes between a `CallResponse` and
//! those functions.
//!
//! The contract is **stateless** — no storage reads, no writes, no `/initialized`
//! bookkeeping beyond the standard guard. The weightmap lives in DIESEL's
//! storage, not here, so this alkane has nothing to corrupt and nothing worth
//! stealing. That is also why every method past `initialize` is a view.

#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::storage::StoragePointer;
use alkanes_support::id::AlkaneId;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};

use crate::abi;
use crate::vm::{Mode, Qualifier};
use generated::FirevectorInterface;

#[derive(Default)]
pub struct Firevector(());

impl AlkaneResponder for Firevector {}

impl Firevector {
    fn weightmap_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/weightmap")
    }
    /// The 12:1 orbital, whose possession authorises `set_weightmap`.
    fn sigil_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/sigil")
    }
    /// DSIGIL, whose possession authorises retrieving that orbital.
    fn claim_authority_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/claim_authority")
    }
    fn orbital_claimed_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/orbital_claimed")
    }

    fn read_id(raw: &[u8]) -> Option<AlkaneId> {
        if raw.len() < 32 {
            return None;
        }
        Some(AlkaneId {
            block: u128::from_le_bytes(raw[0..16].try_into().ok()?),
            tx: u128::from_le_bytes(raw[16..32].try_into().ok()?),
        })
    }

    fn write_id(id: &AlkaneId) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&id.block.to_le_bytes());
        bytes.extend_from_slice(&id.tx.to_le_bytes());
        bytes
    }

    fn sigil(&self) -> Result<AlkaneId> {
        let raw = self.sigil_pointer().get();
        if raw.len() < 32 {
            return Err(anyhow!("governance sigil is not set"));
        }
        Ok(AlkaneId {
            block: u128::from_le_bytes(raw[0..16].try_into()?),
            tx: u128::from_le_bytes(raw[16..32].try_into()?),
        })
    }

    /// Authorisation is **possession only**: the sigil must arrive in
    /// `incoming_alkanes`. There is deliberately no callback to the sigil.
    ///
    /// The canonical `alkanes-std-auth-token::authenticate` returns two tokens
    /// where one went in — it does `CallResponse::forward(&incoming)` and then
    /// pushes the transfer again — so every authentication inflates the supply
    /// of the very thing that is supposed to be unique. cryptoswap-pool hit this
    /// and removed its callback for exactly this reason. Possession-only also
    /// leaves this contract with no outbound call at all, hence no reentrancy
    /// surface.
    fn require_sigil(&self) -> Result<()> {
        let sigil = self.sigil()?;
        let context = self.context()?;
        if !context
            .incoming_alkanes
            .0
            .iter()
            .any(|t| t.id == sigil && t.value > 0)
        {
            return Err(anyhow!("governance sigil not present in incoming alkanes"));
        }
        Ok(())
    }
}

impl FirevectorInterface for Firevector {
    fn initialize(
        &self,
        vector_authority_block: u128,
        vector_authority_tx: u128,
        claim_authority_block: u128,
        claim_authority_tx: u128,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;

        self.sigil_pointer().set(Arc::new(Self::write_id(&AlkaneId {
            block: vector_authority_block,
            tx: vector_authority_tx,
        })));
        self.claim_authority_pointer()
            .set(Arc::new(Self::write_id(&AlkaneId {
                block: claim_authority_block,
                tx: claim_authority_tx,
            })));

        // Seed with the identity vector so the contract is never in a state where
        // it has a sigil but no policy. SPLIT with no qualifier, which reproduces
        // today's equal split among every DIESEL mint exactly.
        self.weightmap_pointer()
            .set(Arc::new(abi::pack_weightmap(&abi::Weightmap {
                mode: Mode::Split,
                qualifier: None,
                rate_floor: 0,
                treasury_bps: 0,
                program: crate::programs::identity(),
            })));

        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    #[allow(clippy::too_many_arguments)]
    fn set_weightmap(
        &self,
        mode: u128,
        has_qualifier: u128,
        qualifier_block: u128,
        qualifier_tx: u128,
        qualifier_opcode: u128,
        rate_floor: u128,
        treasury_bps: u128,
        program: Vec<u128>,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        self.require_sigil()?;

        let weightmap = abi::Weightmap {
            mode: abi::mode_from_word(mode).map_err(|e| anyhow!("{}", e))?,
            qualifier: match has_qualifier {
                0 => None,
                1 => Some(Qualifier {
                    target_block: qualifier_block,
                    target_tx: qualifier_tx,
                    opcode: qualifier_opcode,
                }),
                other => return Err(anyhow!("has_qualifier must be 0 or 1, got {}", other)),
            },
            rate_floor,
            treasury_bps,
            program,
        };

        // Reject anything that cannot run, and anything well-formed that weighs
        // nothing — adopting a vector that qualifies no transaction would halt
        // emission to everybody, which is a governance accident worth blocking at
        // the setter rather than discovering a block later.
        abi::check_adoptable(&weightmap).map_err(|e| anyhow!("{}", e))?;

        self.weightmap_pointer()
            .set(Arc::new(abi::pack_weightmap(&weightmap)));

        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn get_weightmap(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.weightmap_pointer().get().as_ref().clone();
        Ok(response)
    }

    fn get_weightmap_mode(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        // Decoded from the blob rather than read from a second key, so the answer
        // can never disagree with the program it applies to.
        let v = match abi::unpack_weightmap(self.weightmap_pointer().get().as_ref()) {
            Some(w) => w.mode as u128,
            None => abi::MODE_SPLIT,
        };
        response.data = v.to_le_bytes().to_vec();
        Ok(response)
    }

    /// Spend DSIGIL in, receive the 12:1 FIREVECTOR orbital.
    ///
    /// The orbital is minted into this contract's balance at genesis rather than
    /// onto an outpoint, so the emission-policy capability starts with a defined
    /// release path instead of belonging to whoever spends a historic UTXO. This
    /// is that path.
    ///
    /// DSIGIL is returned by `CallResponse::forward`, so claiming does not
    /// consume it and it keeps its DIESEL treasury permission. That separation is
    /// the point: after this call the treasury key and the emission-policy key
    /// are two different tokens that can be held by different parties and handed
    /// to governance on different schedules.
    fn claim_vector_orbital(&self) -> Result<CallResponse> {
        let context = self.context()?;

        let authority = Self::read_id(self.claim_authority_pointer().get().as_ref())
            .filter(|a| *a != AlkaneId { block: 0, tx: 0 })
            .ok_or_else(|| anyhow!("no claim authority is set; the orbital is not claimable"))?;

        if !context
            .incoming_alkanes
            .0
            .iter()
            .any(|t| t.id == authority && t.value > 0)
        {
            return Err(anyhow!(
                "claim authority (DSIGIL) not present in incoming alkanes"
            ));
        }

        // Once. Not because a second claim could mint anything — there is one
        // orbital and it has already left — but so a repeat is an explicit revert
        // rather than a silent no-op that reads as success.
        if !self.orbital_claimed_pointer().get().is_empty() {
            return Err(anyhow!("the vector orbital has already been claimed"));
        }
        self.orbital_claimed_pointer().set(Arc::new(vec![0x01]));

        let orbital = self.sigil()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.alkanes.0.push(AlkaneTransfer {
            id: orbital,
            value: 1,
        });
        Ok(response)
    }

    fn get_claim_authority(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.claim_authority_pointer().get().as_ref().clone();
        Ok(response)
    }

    fn get_sigil(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.sigil_pointer().get().as_ref().clone();
        Ok(response)
    }

    fn evaluate(
        &self,
        mode: u128,
        program: Vec<u128>,
        items: Vec<u128>,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data =
            abi::evaluate_call(mode, &program, &items).map_err(|e| anyhow!("{}", e))?;
        Ok(response)
    }

    fn simulate(&self, mode: u128, program: Vec<u128>, item: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::simulate_call(mode, &program, &item).map_err(|e| anyhow!("{}", e))?;
        Ok(response)
    }

    fn disassemble_program(&self, program: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::disassemble_call(&program).into_bytes();
        Ok(response)
    }

    fn validate_program(&self, mode: u128, program: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::validate_call(mode, &program).into_bytes();
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = String::from("FIREVECTOR").into_bytes();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = String::from("FIREVECTOR").into_bytes();
        Ok(response)
    }
}
