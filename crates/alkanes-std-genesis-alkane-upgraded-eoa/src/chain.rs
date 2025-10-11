use alkanes_support::utils::overflow_error;
use anyhow::Result;

#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};



pub trait ChainConfiguration {
    fn block_reward(&self, n: u64) -> u128;
    fn genesis_block(&self) -> u64;
    fn average_payout_from_genesis(&self) -> u128;
    fn premine(&mut self) -> Result<u128>;
    fn current_block_reward(&mut self) -> u128;
    fn max_supply(&self) -> u128;
}