use crate::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use crate::host::Host;
use anyhow::Result;
use std::collections::BTreeMap;

#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuneTransfer {
    pub id: ProtoruneRuneId,
    pub value: u128,
}

impl RuneTransfer {
    pub fn from_balance_sheet<H: Host + Default>(s: BalanceSheet<H>) -> Vec<Self> {
        s.balances
            .iter()
            .filter_map(|(id, v)| {
                if *v > 0 {
                    Some(RuneTransfer {
                        id: id.clone(),
                        value: *v,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<RuneTransfer>>()
    }
}

pub fn increase_balances_using_sheet<H: Host + Clone + Default>(
    balances_by_output: &mut BTreeMap<u32, BalanceSheet<H>>,
    sheet: &mut BalanceSheet<H>,
    vout: u32,
) -> Result<()> {
    if !balances_by_output.contains_key(&vout) {
        balances_by_output.insert(vout, BalanceSheet::default());
    }
    sheet.pipe(balances_by_output.get_mut(&vout).unwrap())?;
    Ok(())
}

pub fn refund_to_refund_pointer<H: Host + Clone + Default>(
    balances_by_output: &mut BTreeMap<u32, BalanceSheet<H>>,
    protomessage_vout: u32,
    refund_pointer: u32,
) -> Result<()> {
    let sheet = balances_by_output
        .get(&protomessage_vout)
        .map(|v| v.clone())
        .unwrap_or_else(|| BalanceSheet::default());
    balances_by_output.remove(&protomessage_vout);
    increase_balances_using_sheet(balances_by_output, &mut sheet.clone(), refund_pointer)
}
