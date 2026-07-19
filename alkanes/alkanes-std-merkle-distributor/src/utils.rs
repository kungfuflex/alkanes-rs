use alkanes_support::witness::find_witness_payload;
use bitcoin::Transaction;
use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct SchemaMerkleProof {
    pub leaf: Vec<u8>,
    pub proofs: Vec<Vec<u8>>,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct SchemaMerkleLeaf {
    pub address: String,
    pub amount: u128,
}

pub fn extract_witness_payload(tx: &Transaction) -> Option<Vec<u8>> {
    // Try every input; Ordinals conventionally uses index 0, but
    // looping covers edge‑cases.
    for idx in 0..tx.input.len() {
        if let Some(data) = find_witness_payload(&tx, idx) {
            if !data.is_empty() {
                return Some(data);
            }
        }
    }
    None
}

pub fn calc_merkle_root(leaf: &[u8], proofs: &[Vec<u8>]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(leaf);
    let mut node: Vec<u8> = hasher.finalize().to_vec();

    for sib in proofs {
        let (left, right) = if node <= *sib {
            (&node, sib)
        } else {
            (sib, &node)
        };
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        node = hasher.finalize().to_vec();
    }

    // convert Vec<u8> → [u8;32]
    let mut root = [0u8; 32];
    root.copy_from_slice(&node);
    root
}

macro_rules! decode_from_vec {
    ($bytes:expr, $ty:ty) => {{
        use std::io::Cursor;
        // Accept anything that turns into a byte slice; `&Vec<u8>` or `&[u8]` both work.
        let mut rdr = Cursor::new(&$bytes[..]);
        <$ty>::deserialize_reader(&mut rdr)
            .map_err(|_| ::anyhow::anyhow!("failed to decode {}", stringify!($ty)))
    }};
}
pub(crate) use decode_from_vec;
