use alkanes_support::{
    id::AlkaneId,
    proto::alkanes::{self as pb, Payment as ProtoPayment, PendingUnwrapsResponse},
};
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, TxOut};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_core::{get_cache, println, stdio::stdout};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{consensus_decode, consensus_encode, is_empty};
use protorune::tables::OUTPOINT_SPENDABLE_BY;
use std::fmt::Write;
use std::io::Cursor;
use std::sync::Arc;

use crate::network::genesis;

pub fn fr_btc_storage_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/alkanes/")
        .select(&AlkaneId { block: 32, tx: 0 }.into())
        .keyword("/storage/")
}

pub fn fr_btc_fulfilled_pointer() -> IndexPointer {
    fr_btc_storage_pointer().keyword("/fulfilled")
}

pub fn fr_btc_premium() -> u128 {
    let bytes = fr_btc_storage_pointer().keyword("/premium").get();
    if bytes.is_empty() {
        0
    } else {
        u128::from_le_bytes(bytes[0..16].try_into().unwrap())
    }
}

/// Pointer to the precomputed pending payments cache
fn pending_cache_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/__INTERNAL/pending_unwraps")
}

/// Pointer that tracks whether the pending cache has been initialized
fn pending_cache_initialized_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/__INTERNAL/pending_unwraps_initialized")
}

/// Pointer that tracks the last height the pending cache was updated through
fn pending_cache_height_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/__INTERNAL/pending_unwraps_height")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Payment {
    pub spendable: OutPoint,
    pub output: TxOut,
    pub fulfilled: bool,
}

impl Payment {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut result: Vec<u8> = vec![];
        let spendable: Vec<u8> = consensus_encode(&self.spendable)?;
        let output: Vec<u8> = consensus_encode(&self.output)?;
        result.extend(&spendable);
        result.extend(&output);
        Ok(result)
    }
}

impl From<ProtoPayment> for Payment {
    fn from(payment: ProtoPayment) -> Self {
        let spendable = payment.spendable.unwrap();
        let txid = bitcoin::Txid::from_slice(&spendable.txid).unwrap();
        let vout = spendable.vout;
        let output = consensus_decode::<TxOut>(&mut Cursor::new(payment.output)).unwrap();
        Payment {
            spendable: OutPoint { txid, vout },
            output,
            fulfilled: payment.fulfilled,
        }
    }
}

pub fn deserialize_payments(v: &Vec<u8>) -> Result<Vec<Payment>> {
    let mut payments: Vec<Payment> = vec![];
    let mut cursor: Cursor<Vec<u8>> = Cursor::new(v.clone());
    while !is_empty(&mut cursor) {
        let (spendable, output) = (
            consensus_decode::<OutPoint>(&mut cursor)?,
            consensus_decode::<TxOut>(&mut cursor)?,
        );
        payments.push(Payment {
            spendable,
            output,
            fulfilled: false,
        });
    }
    Ok(payments)
}

pub fn fr_btc_payments_at_block(v: u128) -> Vec<Vec<u8>> {
    fr_btc_storage_pointer()
        .keyword("/payments/byheight/")
        .select_value::<u64>(v as u64)
        .get_list()
        .into_iter()
        .map(|v| v.as_ref().clone())
        .collect::<Vec<Vec<u8>>>()
}

/// Check if a payment's spendable outpoint is still unfulfilled
fn is_payment_unfulfilled(payment: &Payment) -> Result<bool> {
    let spendable_bytes = consensus_encode(&payment.spendable)?;
    let spendable_by = OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get();
    Ok(spendable_by.len() > 1)
}

/// Serialize a Payment into bytes for the pending cache
fn serialize_pending_entry(payment: &Payment, block_height: u128) -> Result<Vec<u8>> {
    let mut entry = Vec::new();
    entry.extend(&(block_height as u64).to_le_bytes());
    entry.extend(&consensus_encode(&payment.spendable)?);
    entry.extend(&consensus_encode(&payment.output)?);
    Ok(entry)
}

/// Deserialize a pending cache entry back into (block_height, Payment)
fn deserialize_pending_entry(data: &[u8]) -> Result<(u128, Payment)> {
    if data.len() < 8 {
        return Err(anyhow::anyhow!("pending entry too short"));
    }
    let block_height = u64::from_le_bytes(data[0..8].try_into().unwrap()) as u128;
    let mut cursor = Cursor::new(data[8..].to_vec());
    let spendable = consensus_decode::<OutPoint>(&mut cursor)?;
    let output = consensus_decode::<TxOut>(&mut cursor)?;
    Ok((block_height, Payment { spendable, output, fulfilled: false }))
}

/// Build the pending cache by scanning from last_block to current height.
/// Called lazily on first block after upgrade, or incrementally on each new block.
fn build_pending_cache(height: u128) -> Result<()> {
    let initialized = pending_cache_initialized_pointer().get();
    let cache_ptr = pending_cache_pointer();

    if initialized.is_empty() {
        // First time — do a full scan from last_block to height
        // This happens once on upgrade, then incremental updates after
        println!("Building pending unwraps cache (one-time initialization)...");

        let last_block = std::cmp::max(
            fr_btc_storage_pointer()
                .keyword("/last_block")
                .get_value::<u128>(),
            genesis::GENESIS_BLOCK as u128,
        );

        for i in last_block..=height {
            for payment_list_bytes in fr_btc_payments_at_block(i) {
                let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
                for payment in deserialized_payments {
                    if is_payment_unfulfilled(&payment)? {
                        let entry = serialize_pending_entry(&payment, i)?;
                        cache_ptr.append(Arc::new(entry));
                    }
                }
            }
        }

        // Mark as initialized
        pending_cache_initialized_pointer().set(Arc::new(vec![1u8]));
        pending_cache_height_pointer().set_value::<u64>(height as u64);
        println!("Pending unwraps cache initialized.");
    } else {
        // Incremental update — only scan the new block
        let cache_height = pending_cache_height_pointer().get_value::<u64>() as u128;

        if height > cache_height {
            // Add new pending payments from blocks since last update
            for i in (cache_height + 1)..=height {
                for payment_list_bytes in fr_btc_payments_at_block(i) {
                    let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
                    for payment in deserialized_payments {
                        if is_payment_unfulfilled(&payment)? {
                            let entry = serialize_pending_entry(&payment, i)?;
                            cache_ptr.append(Arc::new(entry));
                        }
                    }
                }
            }

            // Prune fulfilled payments from the cache every 10 blocks
            // to avoid expensive full-list rewrite on every block
            if height % 10 == 0 {
                let list = cache_ptr.get_list();
                let mut kept = Vec::new();
                for entry_arc in &list {
                    if let Ok((blk, payment)) = deserialize_pending_entry(entry_arc.as_ref()) {
                        if is_payment_unfulfilled(&payment)? {
                            kept.push((blk, payment));
                        }
                    }
                }

                // Rewrite the cache with only unfulfilled entries
                let old_len = list.len() as u32;
                cache_ptr.keyword("/length").set_value::<u32>(0);
                for (blk, payment) in &kept {
                    let entry = serialize_pending_entry(payment, *blk)?;
                    cache_ptr.append(Arc::new(entry));
                }
                // Clean up any leftover entries beyond the new length
                let new_len = kept.len() as u32;
                for i in new_len..old_len {
                    cache_ptr.select_index(i).set(Arc::new(vec![]));
                }
            }

            pending_cache_height_pointer().set_value::<u64>(height as u64);
        }
    }

    Ok(())
}

/// Optimized view: reads from precomputed pending cache if available,
/// falls back to full scan if cache not yet initialized.
pub fn view(height: u128) -> Result<PendingUnwrapsResponse> {
    let initialized = pending_cache_initialized_pointer().get();

    if !initialized.is_empty() {
        // Fast path: read from cache
        let cache_ptr = pending_cache_pointer();
        let list = cache_ptr.get_list();
        let mut response = PendingUnwrapsResponse::default();

        for entry_arc in &list {
            if let Ok((_blk, payment)) = deserialize_pending_entry(entry_arc.as_ref()) {
                // Re-check fulfillment status (might have changed since cache was built)
                if is_payment_unfulfilled(&payment)? {
                    response.payments.push(ProtoPayment {
                        spendable: Some(pb::Outpoint {
                            txid: payment.spendable.txid.as_byte_array().to_vec(),
                            vout: payment.spendable.vout,
                        }),
                        output: consensus_encode::<TxOut>(&payment.output)?,
                        fulfilled: false,
                    });
                }
            }
        }

        return Ok(response);
    }

    // Slow path fallback: full scan (only until cache is built)
    let last_block = std::cmp::max(
        fr_btc_storage_pointer()
            .keyword("/last_block")
            .get_value::<u128>(),
        genesis::GENESIS_BLOCK as u128,
    );
    let mut response = PendingUnwrapsResponse::default();
    for i in last_block..=height {
        for payment_list_bytes in fr_btc_payments_at_block(i) {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for mut payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                let spendable_by = OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get();
                if spendable_by.len() <= 1 {
                    payment.fulfilled = true;
                }
                if !payment.fulfilled {
                    response.payments.push(ProtoPayment {
                        spendable: Some(pb::Outpoint {
                            txid: payment.spendable.txid.as_byte_array().to_vec(),
                            vout: payment.spendable.vout,
                        }),
                        output: consensus_encode::<TxOut>(&payment.output)?,
                        fulfilled: payment.fulfilled,
                    });
                }
            }
        }
    }
    Ok(response)
}

/// Called during block indexing to update last_block and maintain the pending cache.
pub fn update_last_block(height: u128) -> Result<()> {
    // Build/update the pending cache
    build_pending_cache(height)?;

    // Original last_block advancement logic
    let mut last_block_key = fr_btc_storage_pointer().keyword("/last_block");
    let mut last_block = std::cmp::max(
        last_block_key.get_value::<u128>(),
        genesis::GENESIS_BLOCK as u128,
    );
    for i in last_block..=height {
        let mut all_fulfilled = true;
        let all_payment_list_bytes = fr_btc_payments_at_block(i);
        if all_payment_list_bytes.len() == 0 {
            last_block = i + 1;
            continue;
        }
        for payment_list_bytes in all_payment_list_bytes {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                let spendable_by = OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get();
                if spendable_by.len() > 1 {
                    all_fulfilled = false;
                    break;
                }
            }
            if !all_fulfilled {
                break;
            }
        }
        if all_fulfilled {
            last_block = i + 1;
        } else {
            break;
        }
    }
    last_block_key.set_value::<u128>(last_block);
    Ok(())
}
