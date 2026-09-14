use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_support::{response::CallResponse, utils::overflow_error};
use anyhow::Result;

#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};

pub struct ContextHandle(());

impl AlkaneResponder for ContextHandle {}

pub const CONTEXT_HANDLE: ContextHandle = ContextHandle(());

pub trait ChainConfiguration {
    fn block_reward(&self, n: u64) -> u128;
    fn genesis_block(&self) -> u64;
    fn average_payout_from_genesis(&self) -> u128;
    fn premine(&self) -> Result<u128> {
        let blocks =
            overflow_error(CONTEXT_HANDLE.height().checked_sub(self.genesis_block()))? as u128;
        Ok(overflow_error(
            blocks.checked_mul(self.average_payout_from_genesis()),
        )?)
    }
    fn current_block_reward(&self) -> u128 {
        self.block_reward(CONTEXT_HANDLE.height())
    }
    fn max_supply(&self) -> u128;
}
