use crate::envelope::RawEnvelope;
use bitcoin::blockdata::transaction::Transaction;

pub fn find_witness_payload(tx: &Transaction, i: usize) -> Option<Vec<u8>> {
    let envelopes = RawEnvelope::from_transaction(tx);
    if envelopes.len() <= i {
        None
    } else {
        Some(
            envelopes[i]
                .payload
                .clone()
                .into_iter()
                .skip(1)
                .flatten()
                .collect(),
        )
    }
}
