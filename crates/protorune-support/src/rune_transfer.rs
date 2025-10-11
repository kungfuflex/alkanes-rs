use std::collections::BTreeMap;

use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::KeyValuePointer;

use crate::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use anyhow::Result;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuneTransfer {
    pub id: ProtoruneRuneId,
    pub value: u128,
}

impl RuneTransfer {
    pub fn from_balance_sheet<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone>(
        s: &BalanceSheet<E, P>,
    ) -> Vec<Self> {
        s.balances()
            .iter()
            .filter_map(|(id, v)| {
                if *v > 0 {
                    Some(RuneTransfer { id: *id, value: *v })
                } else {
                    None
                }
            })
            .collect::<Vec<RuneTransfer>>()
    }
}

/// Parameters:
///   balances_by_output: The running store of balances by each transaction output for
///                       the current transaction being handled.
///   sheet: The balance sheet to increase the balances by
///   vout: The target transaction vout to receive the runes
pub fn increase_balances_using_sheet<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone>(
    balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
    sheet: &BalanceSheet<E, P>,
    vout: u32,
    env: &mut E,
) -> Result<()> {
    if !balances_by_output.contains_key(&vout) {
        balances_by_output.insert(vout, BalanceSheet::default());
    }
    sheet.pipe(balances_by_output.get_mut(&vout).unwrap(), env)?;
    Ok(())
}

/// Refunds all input runes to the refund pointer
pub fn refund_to_refund_pointer<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone>(
    balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, P>>,
    protomessage_vout: u32,
    refund_pointer: u32,
    env: &mut E,
) -> Result<()> {
    let sheet = balances_by_output
        .remove(&protomessage_vout)
        .unwrap_or_default();
    // we want to remove any balance from the protomessage vout
    increase_balances_using_sheet(balances_by_output, &sheet, refund_pointer, env)
}

impl<E: RuntimeEnvironment, P: KeyValuePointer<E> + Clone> TryFrom<BalanceSheet<E, P>>
    for Vec<RuneTransfer>
{
    type Error = anyhow::Error;

    fn try_from(value: BalanceSheet<E, P>) -> Result<Self, Self::Error> {
        Ok(RuneTransfer::from_balance_sheet(&value))
    }
}

