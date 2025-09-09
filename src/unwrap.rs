use alkanes_support::{
    id::AlkaneId,
    proto::alkanes::{self as pb, Payment as ProtoPayment, PendingUnwrapsResponse},
};
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, TxOut};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{consensus_decode, consensus_encode, is_empty};
use protobuf::{MessageField, SpecialFields};
use protorune::tables::OUTPOINT_SPENDABLE_BY;
use std::io::Cursor;
use std::sync::Arc;

pub fn fr_btc_storage_pointer() -> IndexPointer {
    IndexPointer::from_keyword("/alkanes/")
        .select(&AlkaneId { block: 32, tx: 0 }.into())
        .keyword("/storage")
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

#[derive(Debug, Clone, PartialEq)]
pub struct Payment {
    pub spendable: OutPoint,
    pub output: TxOut,
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
        payments.push(Payment { spendable, output });
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
    let last_block = fr_btc_storage_pointer()
        .keyword("/last_block")
        .get_value::<u128>();
    let mut response = PendingUnwrapsResponse::default();
    for i in last_block..=height {
        for payment_list_bytes in fr_btc_payments_at_block(i) {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for mut payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                if OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get().len() == 0 {
                    payment.fulfilled = true;
                }
                if !payment.fulfilled {
                    response.payments.push(ProtoPayment {
                        spendable: MessageField::some(pb::Outpoint {
                            txid: payment.spendable.txid.as_byte_array().to_vec(),
                            vout: payment.spendable.vout,
                            special_fields: SpecialFields::default(),
                        }),
                        output: consensus_encode::<TxOut>(&payment.output)?,
                        fulfilled: payment.fulfilled,
                        special_fields: SpecialFields::default(),
                    });
                }
            }
        }
    }
    Ok(response)
}

pub fn update_last_block(height: u128) -> Result<()> {
    let mut last_block_key = fr_btc_storage_pointer().keyword("/last_block");
    let mut last_block = last_block_key.get_value::<u128>();
    for i in last_block..=height {
        let mut all_fulfilled = true;
        for payment_list_bytes in fr_btc_payments_at_block(i) {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                if OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).get().len() > 0 {
                    all_fulfilled = false;
                    break;
                }
            }
            if !all_fulfilled {
                break;
            }
        }
        if all_fulfilled {
            last_block = i;
        } else {
            break;
        }
    }
    last_block_key.set_value::<u128>(last_block);
    Ok(())
}
