use crate::message::AlkaneMessageContext;
use alkanes_support::{
    id::AlkaneId,
    proto::alkanes::{self as pb, Payment as ProtoPayment, PendingUnwrapsResponse},
};
use anyhow::Result;
use metashrew_support::environment::RuntimeEnvironment;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, TxOut};
use metashrew_support::index_pointer::IndexPointer;

use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{consensus_decode, consensus_encode, is_empty};
use protorune::tables::RuneTable;

use std::io::Cursor;


use crate::network::genesis;

pub fn fr_btc_storage_pointer<E: RuntimeEnvironment>() -> IndexPointer<E> {
    IndexPointer::from_keyword("/fr_btc/storage/")
}

pub fn fr_btc_fulfilled_pointer<E: RuntimeEnvironment>() -> IndexPointer<E> {
    IndexPointer::from_keyword("/fr_btc/fulfilled/")
}

pub fn fr_btc_premium<E: RuntimeEnvironment>(env: &mut E) -> u128 {
    let bytes = fr_btc_storage_pointer::<E>().keyword("/premium").get(env);
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

impl From<Payment> for ProtoPayment {
    fn from(payment: Payment) -> Self {
        ProtoPayment {
            spendable: Some(pb::Outpoint {
                txid: payment.spendable.txid.as_byte_array().to_vec(),
                vout: payment.spendable.vout,
            }),
            output: consensus_encode::<TxOut>(&payment.output).unwrap(),
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

pub fn fr_btc_payments_at_block<E: RuntimeEnvironment>(
    env: &mut E,
    v: u128,
) -> Vec<Vec<u8>> {
    fr_btc_storage_pointer::<E>()
        .keyword("/payments/byheight/")
        .select_value::<u64>(v as u64)
        .get_list(env)
        .into_iter()
        .map(|v| v.as_ref().clone())
        .collect::<Vec<Vec<u8>>>()
}

pub fn view<E: RuntimeEnvironment>(
    env: &mut E,
    height: u128,
) -> Result<PendingUnwrapsResponse> {
    let last_block = std::cmp::max(
        fr_btc_storage_pointer::<E>()
            .keyword("/last_block")
            .get_value(env),
        genesis::GENESIS_BLOCK as u128,
    );
    let mut response = PendingUnwrapsResponse::default();
    for i in last_block..=height {
        for payment_list_bytes in fr_btc_payments_at_block::<E>(env, i) {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for mut payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                if RuneTable::<E>::new()
                    .OUTPOINT_SPENDABLE_BY
                    .select(&spendable_bytes)
                    .get(env)
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

pub fn update_last_block<E: RuntimeEnvironment>(
    env: &mut E,
    height: u128,
) -> Result<()> {
    let mut last_block_key = fr_btc_storage_pointer::<E>().keyword("/last_block");
    let mut last_block = std::cmp::max(last_block_key.get_value(env), genesis::GENESIS_BLOCK as u128);
    for i in last_block..=height {
        let mut all_fulfilled = true;
        let all_payment_list_bytes = fr_btc_payments_at_block::<E>(env, i);
        if all_payment_list_bytes.len() == 0 {
            last_block = i;
            continue;
        }
        for payment_list_bytes in all_payment_list_bytes {
            let deserialized_payments = deserialize_payments(&payment_list_bytes)?;
            for payment in deserialized_payments {
                let spendable_bytes = consensus_encode(&payment.spendable)?;
                let spendable_by = RuneTable::<E>::new()
                    .OUTPOINT_SPENDABLE_BY
                    .select(&spendable_bytes)
                    .get(env);
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
            last_block = i;
        } else {
            break;
        }
    }
    last_block_key.set_value(env, last_block);
    Ok(())
}