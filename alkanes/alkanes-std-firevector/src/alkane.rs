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
use alkanes_support::response::CallResponse;
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
    fn sigil_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/sigil")
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
    fn initialize(&self, sigil_block: u128, sigil_tx: u128) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;

        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&sigil_block.to_le_bytes());
        bytes.extend_from_slice(&sigil_tx.to_le_bytes());
        self.sigil_pointer().set(Arc::new(bytes));

        // Seed with the identity vector so the contract is never in a state where
        // it has a sigil but no policy. SPLIT with no qualifier, which reproduces
        // today's equal split among every DIESEL mint exactly.
        self.weightmap_pointer()
            .set(Arc::new(abi::pack_weightmap(&abi::Weightmap {
                mode: Mode::Split,
                qualifier: None,
                rate_floor: 0,
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
