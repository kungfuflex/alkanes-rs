//! The FIREVECTOR orbital, at 12:1 — the permission to update the emission vector.
//!
//! A single-supply bearer token and nothing else. It has no behaviour to speak
//! of, which is the point: authority is possession, verified by the holder
//! spending it into `set_weightmap` on 12:0, and 12:0 never calls back here.
//!
//! Its blast radius is exactly one thing — the direction of DIESEL emissions.
//! Holding it does not let you mint DIESEL, change the emission schedule, the
//! halving or the supply cap, touch the protocol fee pot, or move anyone's
//! funds. Those all live in the genesis alkane, which FIREVECTOR never modifies.
//!
//! Deliberately NOT the same token as DSIGIL. DSIGIL exists on chain already and
//! carries the DIESEL treasury permission; it is spent into 12:0 once to claim
//! this orbital and is handed straight back. Keeping them separate means the
//! treasury key and the emission-policy key can be held by different parties, and
//! transferred to governance on different schedules.

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
use alkanes_runtime::storage::StoragePointer;
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::Result;
use metashrew_support::index_pointer::KeyValuePointer;

use generated::FvOrbitalInterface;

/// The only supply that will ever exist.
pub const SUPPLY: u128 = 1;

#[derive(Default)]
pub struct FvOrbital(());

impl FvOrbital {
    fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }
}

impl AlkaneResponder for FvOrbital {}

impl FvOrbitalInterface for FvOrbital {
    /// Mint the one unit. `observe_initialization` makes this callable exactly
    /// once, so there is no second path to supply.
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.total_supply_pointer().set_value::<u128>(SUPPLY);
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself,
            value: SUPPLY,
        });

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

    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self
            .total_supply_pointer()
            .get_value::<u128>()
            .to_le_bytes()
            .to_vec();
        Ok(response)
    }
}
