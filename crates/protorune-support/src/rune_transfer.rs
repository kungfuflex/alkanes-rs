use std::collections::HashMap;

use metashrew_support::index_pointer::KeyValuePointer;

use crate::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use anyhow::{anyhow, Result};

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuneTransfer {
    pub id: ProtoruneRuneId,
    pub value: u128,
}

impl RuneTransfer {
    pub fn from_balance_sheet<P: KeyValuePointer + Clone>(s: BalanceSheet<P>) -> Vec<Self> {
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
pub fn increase_balances_using_sheet<P: KeyValuePointer + Clone>(
    balances_by_output: &mut HashMap<u32, BalanceSheet<P>>,
    sheet: &BalanceSheet<P>,
    vout: u32,
) -> Result<()> {
    if !balances_by_output.contains_key(&vout) {
        balances_by_output.insert(vout, BalanceSheet::default());
    }
    sheet.pipe(balances_by_output.get_mut(&vout).unwrap())?;
    Ok(())
}

/// Refunds all input runes to the refund pointer
pub fn refund_to_refund_pointer<P: KeyValuePointer + Clone>(
    balances_by_output: &mut HashMap<u32, BalanceSheet<P>>,
    protomessage_vout: u32,
    refund_pointer: u32,
) -> Result<()> {
    // grab the balance of the protomessage vout
    let sheet = balances_by_output
        .get(&protomessage_vout)
        .map(|v| v.clone())
        .unwrap_or_else(|| BalanceSheet::default());
    // we want to remove any balance from the protomessage vout
    balances_by_output.remove(&protomessage_vout);
    increase_balances_using_sheet(balances_by_output, &sheet, refund_pointer)
}
