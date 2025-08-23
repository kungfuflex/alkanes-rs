use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_core::index_pointer::IndexPointer;
use std::sync::{Arc};
use std::io::{Cursor};
use metashrew_support::utils::{consensus_encode, consensus_decode};

#[allow(unused_imports)]
use {
  metashrew_core::{println, stdio::{stdout}},
  std::fmt::Write
};

pub fn fr_btc_storage_pointer() -> IndexPointer {
  IndexPointer::from_keyword("/alkanes/").select((&AlkaneId {
    block: 32,
    tx: 0
  }).into()).select("/storage")
}

pub fn fr_btc_premium() -> u128 {
  fr_btc_storage_pointer().get_value::<u128>()
}

use bitcoin::{OutPoint, TxOut};

#[derive(Debug, Clone, PartialEq)]
pub struct Payment {
    pub spendable: OutPoint,
    pub output: TxOut,
    pub fulfilled: bool,
}

impl Payment {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut result: Vec<u8> = vec![];
        let spendable: Vec<u8> = consensus_encode::<OutPoint>(&self.spendable)?;
        let output: Vec<u8> = consensus_encode::<TxOut>(&self.output)?;
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
        payments.push(Payment { spendable, output, fulfilled: false });
    }
    Ok(payments)
}

pub fn fr_btc_payments_at_block(v: u128) -> Vec<Vec<u8>> {
  fr_btc_storage_pointer().select(format!("/payments/byheight/{}", v)).get_list().into_iter().map(|v| v.as_ref().clone()).collect::<Vec<Vec<u8>>>()
}


use alkanes_support::proto::alkanes::{PendingUnwrapsResponse, Payment as ProtoPayment};
use protorune::tables::OUTPOINT_SPENDABLE_BY;

pub fn view(height: u128) -> Result<PendingUnwrapsResponse> {
    let last_block = fr_btc_storage_pointer().select(b"/last_block").get_value_or_default::<u128>();
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
                        spendable: Some(payment.spendable.into()),
                        output: consensus_encode(&payment.output)?,
                        fulfilled: payment.fulfilled,
                    });
                }
            }
        }
    }
    Ok(response)
}

use anyhow::{Result};
use alkanes_support::{
  alkane::AlkaneId,
  is_empty,
};

pub fn fr_btc_fulfilled_pointer() -> IndexPointer {
  fr_btc_storage_pointer().select("/fulfilled")
}
