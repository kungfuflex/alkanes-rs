use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};

use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
    utils::{overflow_error, shift},
};
use anyhow::{anyhow, Result};
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_ptr},
    index_pointer::KeyValuePointer,
};
use num::integer::Roots;
use protorune_support::balance_sheet::BalanceSheet;
use ruint::Uint;
use std::sync::Arc;

// per uniswap docs, the first 1e3 wei of lp token minted are burned to mitigate attacks where the value of a lp token is raised too high easily
pub const MINIMUM_LIQUIDITY: u128 = 1000;

type U256 = Uint<256, 4>;

#[derive(Default)]
struct AMMPool(());

pub fn sub_fees(v: u128) -> Result<u128> {
    Ok(overflow_error(v.checked_mul(997))? / 1000)
}

impl AMMPool {
    pub fn alkanes_for_self(&self) -> Result<(AlkaneId, AlkaneId)> {
        Ok((
            StoragePointer::from_keyword("/alkane/0")
                .get()
                .as_ref()
                .clone()
                .try_into()?,
            StoragePointer::from_keyword("/alkane/1")
                .get()
                .as_ref()
                .clone()
                .try_into()?,
        ))
    }
    pub fn check_inputs(
        &self,
        myself: &AlkaneId,
        parcel: &AlkaneTransferParcel,
        n: usize,
    ) -> Result<()> {
        if parcel.0.len() > n {
            Err(anyhow!(format!(
                "{} alkanes sent but maximum {} supported",
                parcel.0.len(),
                n
            )))
        } else {
            let (a, b) = self.alkanes_for_self()?;
            if let Some(_) = parcel
                .0
                .iter()
                .find(|v| myself != &v.id && v.id != a && v.id != b)
            {
                Err(anyhow!("unsupported alkane sent to pool"))
            } else {
                Ok(())
            }
        }
    }
    pub fn total_supply(&self) -> u128 {
        StoragePointer::from_keyword("/totalsupply").get_value::<u128>()
    }
    pub fn set_total_supply(&self, v: u128) {
        StoragePointer::from_keyword("/totalsupply").set_value::<u128>(v);
    }
    pub fn reserves(&self) -> (AlkaneTransfer, AlkaneTransfer) {
        let (a, b) = self.alkanes_for_self().unwrap();
        let context = self.context().unwrap();
        (
            AlkaneTransfer {
                id: a,
                value: self.balance(&context.myself, &a),
            },
            AlkaneTransfer {
                id: b,
                value: self.balance(&context.myself, &b),
            },
        )
    }
    pub fn previous_reserves(
        &self,
        parcel: &AlkaneTransferParcel,
    ) -> (AlkaneTransfer, AlkaneTransfer) {
        let (reserve_a, reserve_b) = self.reserves();
        let mut reserve_sheet: BalanceSheet =
            AlkaneTransferParcel(vec![reserve_a.clone(), reserve_b.clone()]).into();
        let incoming_sheet: BalanceSheet = parcel.clone().into();
        reserve_sheet.debit(&incoming_sheet).unwrap();
        (
            AlkaneTransfer {
                id: reserve_a.id.clone(),
                value: reserve_sheet.get(&reserve_a.id.clone().into()),
            },
            AlkaneTransfer {
                id: reserve_b.id.clone(),
                value: reserve_sheet.get(&reserve_b.id.clone().into()),
            },
        )
    }
    pub fn mint(&self, myself: AlkaneId, parcel: AlkaneTransferParcel) -> Result<CallResponse> {
        self.check_inputs(&myself, &parcel, 2)?;
        let mut total_supply = self.total_supply();
        let (reserve_a, reserve_b) = self.reserves();
        let (previous_a, previous_b) = self.previous_reserves(&parcel);
        let root_k_last = overflow_error(previous_a.value.checked_mul(previous_b.value))?.sqrt();
        let root_k = overflow_error(reserve_a.value.checked_mul(reserve_b.value))?.sqrt();
        if root_k > root_k_last || root_k_last == 0 {
            let liquidity;
            if total_supply == 0 {
                liquidity = overflow_error(root_k.checked_sub(MINIMUM_LIQUIDITY))?;
                total_supply = total_supply + MINIMUM_LIQUIDITY;
            } else {
                let numerator = overflow_error(
                    total_supply.checked_mul(overflow_error(root_k.checked_sub(root_k_last))?),
                )?;
                let denominator = overflow_error(
                    overflow_error(root_k.checked_mul(5))?.checked_add(root_k_last), // constant 5 is assuming 1/6 of LP fees goes as protocol fees
                )?;
                liquidity = numerator / denominator;
            }
            self.set_total_supply(overflow_error(total_supply.checked_add(liquidity))?);
            let mut response = CallResponse::default();
            response.alkanes = AlkaneTransferParcel(vec![AlkaneTransfer {
                id: myself,
                value: liquidity,
            }]);
            Ok(response)
        } else {
            Err(anyhow!("root k is less than previous root k"))
        }
    }
    pub fn burn(&self, myself: AlkaneId, parcel: AlkaneTransferParcel) -> Result<CallResponse> {
        self.check_inputs(&myself, &parcel, 1)?;
        let incoming = parcel.0[0].clone();
        if incoming.id != myself {
            return Err(anyhow!("can only burn LP alkane for this pair"));
        }
        let liquidity = incoming.value;
        let (reserve_a, reserve_b) = self.reserves();
        let total_supply = self.total_supply();
        let mut response = CallResponse::default();
        let amount_a = overflow_error(liquidity.checked_mul(reserve_a.value))? / total_supply;
        let amount_b = overflow_error(liquidity.checked_mul(reserve_b.value))? / total_supply;
        if amount_a == 0 || amount_b == 0 {
            return Err(anyhow!("insufficient liquidity!"));
        }
        self.set_total_supply(overflow_error(total_supply.checked_sub(liquidity))?);
        response.alkanes = AlkaneTransferParcel(vec![
            AlkaneTransfer {
                id: reserve_a.id,
                value: amount_a,
            },
            AlkaneTransfer {
                id: reserve_b.id,
                value: amount_b,
            },
        ]);
        Ok(response)
    }
    pub fn get_amount_out(
        &self,
        amount: u128,
        reserve_from: u128,
        reserve_to: u128,
    ) -> Result<u128> {
        Ok(overflow_error(
            reserve_to.checked_sub(
                (U256::from(reserve_from) * U256::from(reserve_to)
                    / (U256::from(reserve_from) + U256::from(amount)))
                .try_into()?,
            ),
        )?)
    }
    pub fn swap(
        &self,
        parcel: AlkaneTransferParcel,
        amount_out_predicate: u128,
    ) -> Result<CallResponse> {
        if parcel.0.len() != 1 {
            return Err(anyhow!(format!(
                "payload can only include 1 alkane, sent {}",
                parcel.0.len()
            )));
        }
        let transfer = parcel.0[0].clone();
        let (previous_a, previous_b) = self.previous_reserves(&parcel);
        let (reserve_a, reserve_b) = self.reserves();
        let output = if &transfer.id == &reserve_a.id {
            AlkaneTransfer {
                id: reserve_b.id,
                value: sub_fees(self.get_amount_out(
                    transfer.value,
                    previous_a.value,
                    previous_b.value,
                )?)?,
            }
        } else {
            AlkaneTransfer {
                id: reserve_a.id,
                value: sub_fees(self.get_amount_out(
                    transfer.value,
                    previous_b.value,
                    previous_a.value,
                )?)?,
            }
        };
        if output.value < amount_out_predicate {
            return Err(anyhow!("predicate failed: insufficient output"));
        }
        let mut response = CallResponse::default();
        response.alkanes = AlkaneTransferParcel(vec![output]);
        Ok(response)
    }
    pub fn pull_ids(&self, v: &mut Vec<u128>) -> Option<(AlkaneId, AlkaneId)> {
        let a_block = shift(v)?;
        let a_tx = shift(v)?;
        let b_block = shift(v)?;
        let b_tx = shift(v)?;
        Some((AlkaneId::new(a_block, a_tx), AlkaneId::new(b_block, b_tx)))
    }
}

impl AlkaneResponder for AMMPool {
    fn execute(&self) -> CallResponse {
        let context = self.context().unwrap();
        let mut inputs = context.inputs.clone();
        match shift(&mut inputs).unwrap() {
            0 => {
                let mut pointer = StoragePointer::from_keyword("/initialized");
                if pointer.get().len() == 0 {
                    pointer.set(Arc::new(vec![0x01]));
                    let (a, b) = self.pull_ids(&mut inputs).unwrap();
                    StoragePointer::from_keyword("/alkane/0").set(Arc::new(a.into()));
                    StoragePointer::from_keyword("/alkane/1").set(Arc::new(b.into()));
                    self.mint(context.myself, context.incoming_alkanes).unwrap()
                } else {
                    panic!("already initialized");
                }
            }
            1 => self.mint(context.myself, context.incoming_alkanes).unwrap(),
            2 => self.burn(context.myself, context.incoming_alkanes).unwrap(),
            3 => self
                .swap(context.incoming_alkanes, shift(&mut inputs).unwrap())
                .unwrap(),
            50 => CallResponse::forward(&context.incoming_alkanes),

            _ => {
                panic!("unrecognized opcode");
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn __execute() -> i32 {
    let mut response = to_arraybuffer_layout(&AMMPool::default().run());
    to_ptr(&mut response) + 4
}
