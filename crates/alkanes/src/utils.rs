use anyhow::anyhow;
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer, KeyValuePointer};
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::{AlkaneTransfer, AlkaneTransferParcel};
use alkanes_support::storage::StorageMap;
use protorune_support::rune_transfer::RuneTransfer;
use std::sync::Arc;
use bitcoin::OutPoint;
use std::io::Cursor;
use metashrew_support::utils::consensus_decode;

pub fn balance_pointer(
    atomic: &mut AtomicPointer,
    who: &AlkaneId,
    what: &AlkaneId,
) -> AtomicPointer {
    let who_bytes: Vec<u8> = who.clone().into();
    let what_bytes: Vec<u8> = what.clone().into();
    let ptr = atomic
        .derive(&IndexPointer::default())
        .keyword("/alkanes/")
        .select(&what_bytes)
        .keyword("/balances/")
        .select(&who_bytes);
    if ptr.get().len() != 0 {
        alkane_inventory_pointer(who).append(Arc::new(what_bytes));
    }
    ptr
}

pub fn alkane_inventory_pointer(who: &AlkaneId) -> IndexPointer {
    let who_bytes: Vec<u8> = who.clone().into();
    let ptr = IndexPointer::from_keyword("/alkanes/")
        .select(&who_bytes)
        .keyword("/inventory/");
    ptr
}
pub fn alkane_id_to_outpoint(
    alkane_id: &AlkaneId,
) -> Result<OutPoint, anyhow::Error> {
    let alkane_id_bytes: Vec<u8> = alkane_id.clone().into();
    let outpoint_bytes = IndexPointer::from_keyword(
        "/alkanes_id_to_outpoint/",
    )
    .select(&alkane_id_bytes)
    .get()
    .as_ref()
    .clone();
    if outpoint_bytes.len() == 0 {
        return Err(anyhow!("No creation outpoint for alkane id"));
    }
    let outpoint = consensus_decode::<OutPoint>(&mut Cursor::new(outpoint_bytes))?;
    Ok(outpoint)
}

pub fn credit_balances(
    atomic: &mut AtomicPointer,
    who: &AlkaneId,
    runes: &Vec<RuneTransfer>,
) -> Result<(), anyhow::Error> {
    for rune in runes.clone() {
        let mut ptr = balance_pointer(atomic, who, &rune.id.clone().into());
        let value = ptr.get_value::<u128>();
        ptr.set_value::<u128>(
            rune.value
                .checked_add(value)
                .ok_or("")
                .map_err(|_| anyhow!("balance overflow during credit_balances"))?,
        );
    }
    Ok(())
}pub fn checked_debit_with_minting(
    transfer: &AlkaneTransfer,
    from: &AlkaneId,
    balance: u128,
) -> Result<u128, anyhow::Error> {
    // NOTE: we intentionally allow alkanes to mint an infinite amount of themselves
    // It is up to the contract creator to ensure that this functionality is not abused.
    // Alkanes should not be able to arbitrarily mint alkanes that is not itself
    let mut this_balance = balance;
    if balance < transfer.value {
        if &transfer.id == from {
            this_balance = transfer.value;
        } else {
            return Err(anyhow!(
                "balance underflow, transferring({:?}), from({:?}), balance({})",
                transfer, from, balance
            ));
        }
    }
    Ok(this_balance - transfer.value)
}

pub fn debit_balances(
    atomic: &mut AtomicPointer,
    who: &AlkaneId,
    runes: &AlkaneTransferParcel,
) -> Result<(), anyhow::Error> {
    for transfer in &runes.0 {
        let mut pointer = balance_pointer(atomic, who, &transfer.id.clone().into());
        let pointer_value = pointer.get_value::<u128>();
        pointer.set_value::<u128>(checked_debit_with_minting(transfer, who, pointer_value)?);
    }
    Ok(())
}
pub fn transfer_from(
    runes: &AlkaneTransferParcel,
    atomic: &mut AtomicPointer,
    from: &AlkaneId,
    to: &AlkaneId,
) -> Result<(), anyhow::Error> {
    let non_contract_id = AlkaneId { block: 0, tx: 0 };
    if *to == non_contract_id {
        println!("skipping transfer_from since caller is not a contract");
        return Ok(());
    }
    for transfer in &runes.0 {
        let mut from_pointer =
            balance_pointer(atomic, &from.clone().into(), &transfer.id.clone().into());
        let balance = from_pointer.get_value::<u128>();
        from_pointer.set_value::<u128>(checked_debit_with_minting(transfer, from, balance)?);
        let mut to_pointer =
            balance_pointer(atomic, &to.clone().into(), &transfer.id.clone().into());
        let value = to_pointer.get_value::<u128>();
        to_pointer.set_value::<u128>(value + transfer.value);
    }
    Ok(())
}
pub fn pipe_storagemap_to(
    from: &StorageMap,
    to: &mut AtomicPointer,
) {
    from.0.iter().for_each(|(k, v)| {
        to
            .keyword("/storage/")
            .select(k)
            .set(Arc::new(v.clone()));
    });
}