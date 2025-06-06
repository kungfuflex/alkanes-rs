use crate::{
    message::AlkaneMessageContext,
    vm::{AlkanesInstance, AlkanesState},
};
use alkanes_support::utils::overflow_error;
use anyhow::{anyhow, Result};
use bitcoin::{Block, Transaction, Witness};
use ordinals::{Artifact, Runestone};
use protorune::message::MessageContext;
use protorune_support::protostone::Protostone;
use protorune_support::utils::decode_varint_list;
use ruint::aliases::U32;
use std::io::Cursor;
use std::sync::RwLock;
use wasmi::*;

#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

pub trait VirtualFuelBytes {
    fn vfsize(&self) -> u64;
}

impl VirtualFuelBytes for Transaction {
    fn vfsize(&self) -> u64 {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(&self) {
            if let Ok(protostones) = Protostone::from_runestone(runestone) {
                let cellpacks = protostones
                    .iter()
                    .filter_map(|v| {
                        if v.protocol_tag == AlkaneMessageContext::protocol_tag() {
                            decode_varint_list(&mut Cursor::new(v.message.clone()))
                                .and_then(|list| {
                                    if list.len() >= 2 {
                                        Ok(Some(list))
                                    } else {
                                        Ok(None)
                                    }
                                })
                                .unwrap_or_else(|_| None)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<Vec<u128>>>();
                if cellpacks.len() == 0 {
                    0
                } else if cellpacks
                    .iter()
                    .position(|v| {
                        <&[u128] as TryInto<[u128; 2]>>::try_into(&v[0..2]).unwrap()
                            == [1u128, 0u128]
                            || v[0] == 3u128
                    })
                    .is_some()
                {
                    let mut cloned = self.clone();
                    if cloned.input.len() > 0 {
                        cloned.input[0].witness = Witness::new();
                    }
                    cloned.vsize() as u64
                } else {
                    self.vsize() as u64
                }
            } else {
                0
            }
        } else {
            0
        }
    }
}

impl VirtualFuelBytes for Block {
    fn vfsize(&self) -> u64 {
        self.txdata.iter().fold(0u64, |r, v| r + v.vfsize())
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
pub const FUEL_CHANGE1_HEIGHT: u32 = 0;
#[cfg(feature = "mainnet")]
pub const FUEL_CHANGE1_HEIGHT: u32 = 899_087;
#[cfg(feature = "dogecoin")]
pub const FUEL_CHANGE1_HEIGHT: u32 = 5_730_675;
#[cfg(feature = "fractal")]
pub const FUEL_CHANGE1_HEIGHT: u32 = 759_865;
#[cfg(feature = "luckycoin")]
pub const FUEL_CHANGE1_HEIGHT: u32 = 1_664_317;
#[cfg(feature = "bellscoin")]
pub const FUEL_CHANGE1_HEIGHT: u32 = 533_970;

//use if regtest
#[cfg(not(any(
    feature = "mainnet",
    feature = "dogecoin",
    feature = "bellscoin",
    feature = "fractal",
    feature = "luckycoin"
)))]
pub const TOTAL_FUEL_START: u64 = 100_000_000;
#[cfg(feature = "mainnet")]
pub const TOTAL_FUEL_START: u64 = 100_000_000;
#[cfg(feature = "dogecoin")]
pub const TOTAL_FUEL_START: u64 = 60_000_000;
#[cfg(feature = "fractal")]
pub const TOTAL_FUEL_START: u64 = 50_000_000;
#[cfg(feature = "luckycoin")]
pub const TOTAL_FUEL_START: u64 = 50_000_000;
#[cfg(feature = "bellscoin")]
pub const TOTAL_FUEL_START: u64 = 50_000_000;

//use if regtest
#[cfg(not(any(
    feature = "mainnet",
    feature = "dogecoin",
    feature = "bellscoin",
    feature = "fractal",
    feature = "luckycoin"
)))]
pub const TOTAL_FUEL_CHANGE1: u64 = 1_000_000_000;
#[cfg(feature = "mainnet")]
pub const TOTAL_FUEL_CHANGE1: u64 = 1_000_000_000;
#[cfg(feature = "dogecoin")]
pub const TOTAL_FUEL_CHANGE1: u64 = 600_000_000;
#[cfg(feature = "fractal")]
pub const TOTAL_FUEL_CHANGE1: u64 = 500_000_000;
#[cfg(feature = "luckycoin")]
pub const TOTAL_FUEL_CHANGE1: u64 = 500_000_000;
#[cfg(feature = "bellscoin")]
pub const TOTAL_FUEL_CHANGE1: u64 = 500_000_000;

pub const fn total_fuel(height: u32) -> u64 {
    if height >= FUEL_CHANGE1_HEIGHT {
        TOTAL_FUEL_CHANGE1
    } else {
        TOTAL_FUEL_START
    }
}

#[derive(Default, Clone, Debug)]
pub struct FuelTank {
    pub current_txindex: u32,
    pub size: u64,
    pub txsize: u64,
    pub block_fuel: u64,
    pub transaction_fuel: u64,
    pub block_metered_fuel: u64,
}

static _FUEL_TANK: RwLock<Option<FuelTank>> = RwLock::new(None);

impl FuelTank {
    pub fn get_fuel_tank_copy() -> Option<FuelTank> {
        _FUEL_TANK.read().unwrap().clone()
    }

    pub fn should_advance(txindex: u32) -> bool {
        _FUEL_TANK.read().unwrap().as_ref().unwrap().current_txindex != txindex
    }

    pub fn is_top() -> bool {
        _FUEL_TANK.read().unwrap().as_ref().unwrap().current_txindex == u32::MAX
    }

    pub fn initialize(block: &Block, height: u32) {
        let mut tank = _FUEL_TANK.write().unwrap();
        *tank = Some(FuelTank {
            current_txindex: u32::MAX,
            txsize: 0,
            size: block.vfsize(),
            block_fuel: total_fuel(height),
            transaction_fuel: 0,
            block_metered_fuel: 0,
        });
    }

    #[cfg(not(any(
        feature = "mainnet",
        feature = "dogecoin",
        feature = "bellscoin",
        feature = "fractal",
        feature = "luckycoin"
    )))]
    pub fn _calculate_transaction_fuel(tank: &FuelTank, height: u32) -> u64 {
        // for testing it is useful to assume we always get minimum fuel
        std::cmp::max(minimum_fuel(height), tank.block_metered_fuel)
    }

    #[cfg(any(
        feature = "mainnet",
        feature = "dogecoin",
        feature = "bellscoin",
        feature = "fractal",
        feature = "luckycoin"
    ))]
    pub fn _calculate_transaction_fuel(tank: &FuelTank, height: u32) -> u64 {
        std::cmp::max(minimum_fuel(height), tank.block_metered_fuel)
    }

    pub fn fuel_transaction(txsize: u64, txindex: u32, height: u32) {
        let mut tank = _FUEL_TANK.write().unwrap();
        let tank = tank.as_mut().unwrap();
        tank.current_txindex = txindex;

        // Calculate fuel allocation based on transaction size
        let _block_fuel_before = tank.block_fuel;
        tank.block_metered_fuel = tank.block_fuel * txsize / tank.size;

        tank.transaction_fuel = FuelTank::_calculate_transaction_fuel(&tank, height);

        // Deduct allocated fuel from block fuel
        tank.block_fuel = tank.block_fuel - std::cmp::min(tank.block_fuel, tank.block_metered_fuel);
        tank.txsize = txsize;

        #[cfg(feature = "debug-log")]
        {
            println!("Fuel allocation for transaction {}:", txindex);
            println!("  - Transaction size: {} bytes", txsize);
            println!("  - Block size: {} bytes", tank.size);
            println!("  - Block fuel before: {}", _block_fuel_before);
            println!("  - Block fuel after: {}", tank.block_fuel);
            println!("  - Allocated fuel: {}", tank.transaction_fuel);
            println!("  - Minimum fuel: {}", minimum_fuel(height));
        }
    }

    pub fn refuel_block() {
        let mut tank = _FUEL_TANK.write().unwrap();
        let tank = tank.as_mut().unwrap();

        #[cfg(feature = "debug-log")]
        {
            // Log refunding details before refunding
            println!(
                "Refunding fuel to block after transaction {}:",
                tank.current_txindex
            );
            println!("  - Block fuel before refund: {}", tank.block_fuel);
            println!("  - Remaining metered fuel: {}", tank.block_metered_fuel);
            println!("  - Transaction size: {} bytes", tank.txsize);
            println!("  - Block size before update: {} bytes", tank.size);
        }

        // Only refund the remaining fuel (block_metered_fuel) that wasn't consumed
        // This value is updated by consume_fuel() to reflect the remaining amount
        // after transaction execution
        tank.block_fuel = tank.block_fuel + tank.block_metered_fuel;
        tank.size = tank.size - tank.txsize;

        #[cfg(feature = "debug-log")]
        {
            // Log refunding details after refunding
            println!("  - Block fuel after refund: {}", tank.block_fuel);
            println!("  - Block size after update: {} bytes", tank.size);
        }
    }

    pub fn consume_fuel(n: u64) -> Result<()> {
        let mut tank = _FUEL_TANK.write().unwrap();
        let tank = tank.as_mut().unwrap();

        // Check if we have enough transaction_fuel
        if tank.transaction_fuel < n {
            // Add detailed logging for fuel exhaustion
            return Err(anyhow!(
                "all fuel consumed by WebAssembly: requested {} units, but only {} remaining. \
                Transaction index: {}, Block metered fuel: {}, Block fuel remaining: {}, \
                Transaction size: {} bytes, Block size: {} bytes",
                n,
                tank.transaction_fuel,
                tank.current_txindex,
                tank.block_metered_fuel,
                tank.block_fuel,
                tank.txsize,
                tank.size
            ));
        }

        // Update transaction_fuel - this is used to check if we have enough fuel
        tank.transaction_fuel = tank.transaction_fuel - n;

        // Update block_metered_fuel - this is the amount that will be refunded to the block
        // If we don't have enough block_metered_fuel, set it to 0 (no refund)
        // This ensures we don't refund more fuel than was allocated
        tank.block_metered_fuel = tank.block_metered_fuel.checked_sub(n).unwrap_or(0);

        Ok(())
    }

    pub fn drain_fuel() {
        let mut tank = _FUEL_TANK.write().unwrap();
        let tank = tank.as_mut().unwrap();
        // Don't subtract from block_fuel since we're not refunding in error case
        tank.transaction_fuel = 0;
        tank.block_metered_fuel = 0;
    }

    pub fn start_fuel() -> u64 {
        _FUEL_TANK
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .transaction_fuel
    }
}

pub const MINIMUM_FUEL_START: u64 = 350_000;
pub const MINIMUM_FUEL_CHANGE1: u64 = 3_500_000;
pub const fn minimum_fuel(height: u32) -> u64 {
    if height >= FUEL_CHANGE1_HEIGHT {
        MINIMUM_FUEL_CHANGE1
    } else {
        MINIMUM_FUEL_START
    }
}
pub const FUEL_PER_REQUEST_BYTE: u64 = 1;
pub const FUEL_PER_LOAD_BYTE: u64 = 2;
pub const FUEL_PER_STORE_BYTE_START: u64 = 8;
pub const FUEL_PER_STORE_BYTE_CHANGE1: u64 = 40;
pub const fn fuel_per_store_byte(height: u32) -> u64 {
    if height >= FUEL_CHANGE1_HEIGHT {
        FUEL_PER_STORE_BYTE_CHANGE1
    } else {
        FUEL_PER_STORE_BYTE_START
    }
}

pub const FUEL_SEQUENCE: u64 = 5;
pub const FUEL_FUEL: u64 = 5;
pub const FUEL_EXTCALL: u64 = 500;
pub const FUEL_HEIGHT: u64 = 10;
pub const FUEL_BALANCE: u64 = 10;
pub const FUEL_EXTCALL_DEPLOY_START: u64 = 10_000;
pub const FUEL_EXTCALL_DEPLOY_CHANGE1: u64 = 100_000;
pub const fn fuel_extcall_deploy(height: u32) -> u64 {
    if height >= FUEL_CHANGE1_HEIGHT {
        FUEL_EXTCALL_DEPLOY_CHANGE1
    } else {
        FUEL_EXTCALL_DEPLOY_START
    }
}
pub const FUEL_LOAD_BLOCK: u64 = 1000; // Fixed cost for loading a block
pub const FUEL_LOAD_TRANSACTION: u64 = 500; // Fixed cost for loading a transaction

pub trait Fuelable {
    fn consume_fuel(&mut self, n: u64) -> Result<()>;
}

impl<'a> Fuelable for Caller<'_, AlkanesState> {
    fn consume_fuel(&mut self, n: u64) -> Result<()> {
        overflow_error((self.get_fuel().unwrap() as u64).checked_sub(n))?;
        Ok(())
    }
}

impl Fuelable for AlkanesInstance {
    fn consume_fuel(&mut self, n: u64) -> Result<()> {
        overflow_error((self.store.get_fuel().unwrap() as u64).checked_sub(n))?;
        Ok(())
    }
}

pub fn consume_fuel<'a>(caller: &mut Caller<'_, AlkanesState>, n: u64) -> Result<()> {
    caller.consume_fuel(n)
}

pub fn compute_extcall_fuel(savecount: u64, height: u32) -> Result<u64> {
    let save_fuel = overflow_error(fuel_per_store_byte(height).checked_mul(savecount))?;
    overflow_error::<u64>(FUEL_EXTCALL.checked_add(save_fuel))
}
