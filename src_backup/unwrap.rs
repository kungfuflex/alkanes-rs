use anyhow::Result;
use bitcoin::{OutPoint, TxOut};
use std::io::Cursor;
use std::sync::Arc;
use metashrew_support::index_pointer::KeyValuePointer;
use bitcoin::hashes::Hash;

use alkanes_support::{
    id::AlkaneId,
    proto::alkanes::{self as pb, Payment as ProtoPayment, PendingUnwrapsResponse},
};
use metashrew_support::utils::{is_empty, consensus_encode, consensus_decode};
use metashrew_core::index_pointer::IndexPointer;
use protorune::tables::OUTPOINT_SPENDABLE_BY;

pub fn fr_btc_storage_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/alkanes/")
        .select(&AlkaneId { block: 32, tx: 0 }.into())
        .keyword("/storage")
}

pub fn fr_btc_fulfilled_pointer() -> IndexPointer {
    fr_btc_storage_pointer().keyword("/fulfilled")
}

pub fn fr_btc_premium() -> u128 {
    let bytes = fr_btc_storage_pointer()
        .keyword("/premium")
        .get();
    if bytes.is_empty() {
        0
    } else {
        u128::from_le_bytes(bytes[0..16].try_into().unwrap())
    }
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
        .select(&format!("/payments/byheight/{}", v).as_bytes().to_vec())
        .get_list()
        .into_iter()
        .map(|v| v.as_ref().clone())
        .collect::<Vec<Vec<u8>>>()
}

pub fn view(height: u128) -> Result<PendingUnwrapsResponse> {
    let last_block_bytes = fr_btc_storage_pointer()
        .keyword("/last_block")
        .get();
    let last_block = if last_block_bytes.is_empty() {
        0u128
    } else {
        u128::from_le_bytes(last_block_bytes[0..16].try_into().unwrap())
    };
    let mut response = PendingUnwrapsResponse::default();
    for i in last_block..=height {
        for payment_list_bytes in fr_btc_payments_at_block(i) {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for mut payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                if OUTPOINT_SPENDABLE_BY
                    .select(&spendable_bytes)
                    .get()
                    .len()
                    == 0
                {
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
