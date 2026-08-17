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
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::response::CallResponse;
use anyhow::{anyhow, Result};

use crate::abi;
use generated::FirevectorInterface;

#[derive(Default)]
pub struct Firevector(());

impl AlkaneResponder for Firevector {}

impl FirevectorInterface for Firevector {
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn evaluate(
        &self,
        source: u128,
        program: Vec<u128>,
        items: Vec<u128>,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data =
            abi::evaluate_call(source, &program, &items).map_err(|e| anyhow!("{}", e))?;
        Ok(response)
    }

    fn simulate(&self, source: u128, program: Vec<u128>, item: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::simulate_call(source, &program, &item).map_err(|e| anyhow!("{}", e))?;
        Ok(response)
    }

    fn disassemble_program(&self, program: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::disassemble_call(&program).into_bytes();
        Ok(response)
    }

    fn validate_program(&self, source: u128, program: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = abi::validate_call(source, &program).into_bytes();
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
