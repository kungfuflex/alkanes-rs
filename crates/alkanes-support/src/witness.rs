use crate::envelope::RawEnvelope;
use bitcoin::blockdata::transaction::Transaction;

pub fn find_witness_payload(tx: &Transaction, i: usize) -> Option<Vec<u8>> {
    let envelopes = RawEnvelope::from_transaction(tx);
    if envelopes.len() <= i {
        None
    } else {
        // Don't skip any elements - the payload contains the actual binary data
        // The skip(1) was for a different envelope format
        Some(
            envelopes[i]
                .payload
                .clone()
                .into_iter()
                .flatten()
                .collect(),
        )
    }
}
