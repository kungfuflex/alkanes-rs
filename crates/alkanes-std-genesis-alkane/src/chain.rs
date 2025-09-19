use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_support::utils::overflow_error;
use anyhow::Result;

#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};

pub trait ChainConfiguration: AlkaneResponder {
    fn block_reward(&mut self, n: u64) -> u128;
    fn genesis_block(&mut self) -> u64;
    fn average_payout_from_genesis(&mut self) -> u128;
    fn premine(&mut self) -> Result<u128> {
        let blocks = overflow_error(self.height().checked_sub(self.genesis_block()))? as u128;
        Ok(overflow_error(
            blocks.checked_mul(self.average_payout_from_genesis()),
        )?)
    }
    fn current_block_reward(&mut self) -> u128 {
        let height = self.height();
        self.block_reward(height)
    }
    fn max_supply(&mut self) -> u128;
}
