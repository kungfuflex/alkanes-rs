use alkanes_support::parcel::AlkaneTransferParcel;
use alkanes_support::storage::StorageMap;
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer};
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::consensus_decode;
use std::io::Cursor;
use std::sync::Arc;

pub fn from_protobuf(v: alkanes_support::proto::alkanes::AlkaneId) -> AlkaneId {
    AlkaneId {
        block: v.block.unwrap().into(),
        tx: v.tx.unwrap().into(),
    }
}

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

pub fn alkane_id_to_outpoint(alkane_id: &AlkaneId) -> Result<OutPoint> {
    let alkane_id_bytes: Vec<u8> = alkane_id.clone().into();
    let outpoint_bytes = IndexPointer::from_keyword("/alkanes_id_to_outpoint/")
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
    to: &AlkaneId,
    runes: &Vec<RuneTransfer>,
) -> Result<()> {
    for rune in runes.clone() {
        let mut ptr = balance_pointer(atomic, to, &rune.id.clone().into());
        ptr.set_value::<u128>(
            rune.value
                .checked_add(ptr.get_value::<u128>())
                .ok_or("")
                .map_err(|_| anyhow!("balance overflow during credit_balances"))?,
        );
    }
    Ok(())
}

pub fn checked_debit_with_minting(
    transfer: &AlkaneTransfer,
    from: &AlkaneId,
    balance: u128,
) -> Result<u128> {
    // NOTE: we intentionally allow alkanes to mint an infinite amount of themselves
    // It is up to the contract creator to ensure that this functionality is not abused.
    // Alkanes should not be able to arbitrarily mint alkanes that is not itself
    let mut this_balance = balance;
    if balance < transfer.value {
        if &transfer.id == from {
            this_balance = transfer.value;
        } else {
            return Err(anyhow!(format!(
                "balance underflow, transferring({:?}), from({:?}), balance({})",
                transfer, from, balance
            )));
        }
    }
    Ok(this_balance - transfer.value)
}

pub fn debit_balances(
    atomic: &mut AtomicPointer,
    to: &AlkaneId,
    runes: &AlkaneTransferParcel,
) -> Result<()> {
    for transfer in &runes.0 {
        let mut pointer = balance_pointer(atomic, to, &transfer.id.clone().into());
        let pointer_value = pointer.get_value::<u128>();
        pointer.set_value::<u128>(checked_debit_with_minting(transfer, to, pointer_value)?);
    }
    Ok(())
}

pub fn transfer_from(
    parcel: &AlkaneTransferParcel,
    atomic: &mut AtomicPointer,
    from: &AlkaneId,
    to: &AlkaneId,
) -> Result<()> {
    let non_contract_id = AlkaneId { block: 0, tx: 0 };
    if *to == non_contract_id {
        println!("skipping transfer_from since caller is not a contract");
        return Ok(());
    }
    for transfer in &parcel.0 {
        let mut from_pointer =
            balance_pointer(atomic, &from.clone().into(), &transfer.id.clone().into());
        let balance = from_pointer.get_value::<u128>();
        from_pointer.set_value::<u128>(checked_debit_with_minting(transfer, from, balance)?);
        let mut to_pointer =
            balance_pointer(atomic, &to.clone().into(), &transfer.id.clone().into());
        to_pointer.set_value::<u128>(to_pointer.get_value::<u128>() + transfer.value);
    }
    Ok(())
}
pub fn pipe_storagemap_to<T: KeyValuePointer>(map: &StorageMap, pointer: &mut T) {
    map.0.iter().for_each(|(k, v)| {
        pointer
            .keyword("/storage/")
            .select(k)
            .set(Arc::new(v.clone()));
    });
}
