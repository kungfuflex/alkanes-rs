//! Test-only DIESEL mint gate (tests/mint_gate.rs). Models the SUBMINER-GATE
//! interface DIESEL v3 relies on: DIESEL calls opcode 1 (mint-split) with the
//! freshly minted DIESEL as incoming_alkanes. Behaviour is switchable so the
//! tests can exercise every branch of DIESEL's gate handling:
//!   mode 0 (default): return half of the incoming DIESEL, keep the rest;
//!   mode 1: revert;
//!   mode 2: return the incoming DIESEL PLUS everything previously retained
//!           (i.e. more than was minted);
//!   mode 3: spin until out of fuel (a gate that burns its whole allotment).
//! Opcode 1 also records the full calldata it was called with (opcode 4 reads
//! it back) so a test can prove DIESEL, not the minter, chose the opcode.
use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };

#[derive(Default)]
pub struct TestMintGate(());

#[derive(MessageDispatch)]
enum TestMintGateMessage {
    #[opcode(0)]
    Initialize,
    #[opcode(1)]
    MintSplit,
    #[opcode(2)]
    SetMode { mode: u128 },
    #[opcode(3)]
    #[returns(u128)]
    GetCalls,
    #[opcode(4)]
    #[returns(Vec<u8>)]
    GetLastInputs,
    #[opcode(102)]
    #[returns(Vec<u8>)]
    View,
}

impl TestMintGate {
    fn mode(&self) -> u128 {
        StoragePointer::from_keyword("/mode").get_value::<u128>()
    }
    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
    fn mint_split(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut calls = StoragePointer::from_keyword("/calls");
        let n = calls.get_value::<u128>();
        calls.set_value::<u128>(n + 1);
        let mut inputs = Vec::new();
        for w in context.inputs.iter() {
            inputs.extend_from_slice(&w.to_le_bytes());
        }
        StoragePointer::from_keyword("/last-inputs").set(Arc::new(inputs));
        let minted: u128 = context
            .incoming_alkanes
            .0
            .iter()
            .filter(|t| t.id == DIESEL)
            .map(|t| t.value)
            .sum();
        let mut response = CallResponse::default();
        match self.mode() {
            1 => return Err(anyhow!("test gate: forced revert")),
            3 => {
                // Write storage forever: every write costs fuel, so this traps
                // with out-of-fuel, and the optimizer can't drop it.
                let mut spin = StoragePointer::from_keyword("/spin");
                loop {
                    let v = spin.get_value::<u128>();
                    spin.set_value::<u128>(v.wrapping_add(1));
                }
            }
            2 => {
                let held = self.balance(&context.myself, &DIESEL);
                response.alkanes.0.push(AlkaneTransfer { id: DIESEL, value: held });
            }
            _ => {
                response.alkanes.0.push(AlkaneTransfer { id: DIESEL, value: minted / 2 });
            }
        }
        Ok(response)
    }
    fn set_mode(&self, mode: u128) -> Result<CallResponse> {
        let context = self.context()?;
        StoragePointer::from_keyword("/mode").set_value::<u128>(mode);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
    fn get_calls(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = StoragePointer::from_keyword("/calls")
            .get_value::<u128>()
            .to_le_bytes()
            .to_vec();
        Ok(response)
    }
    fn get_last_inputs(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = StoragePointer::from_keyword("/last-inputs").get().as_ref().clone();
        Ok(response)
    }
    /// A pass-through view: the opcode a hostile minter would like to select
    /// to get the whole mint back unsplit.
    fn view(&self) -> Result<CallResponse> {
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
}

impl AlkaneResponder for TestMintGate {}

declare_alkane! {
    impl AlkaneResponder for TestMintGate {
        type Message = TestMintGateMessage;
    }
}
