use anyhow::{anyhow, Result};
use bitcoin;
use bitcoin::consensus::encode::serialize;
use bitcoin::hashes::Hash;
use bitcoin::consensus::encode::Decodable;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::sync::Arc;

pub static BLOCKS: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/blockdata/"));
pub static TRANSACTIONS: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/transactiondata/"));

pub fn index_extensions(height: u32, v: &bitcoin::Block) {
    BLOCKS.select_value(height).set(Arc::new(serialize(v)));
    for tx in &v.txdata {
        TRANSACTIONS
            .select(&tx.compute_txid().to_byte_array().to_vec())
            .set(Arc::new(serialize(tx)));
    }
}

pub fn get_block(height: u32) -> Result<bitcoin::Block> {
    let block_data = BLOCKS.select_value(height).get();
    if block_data.len() == 0 {
        return Err(anyhow!("Block not found for height: {}", height));
    }

    let mut cursor = Cursor::new(block_data.as_ref().to_vec());
    bitcoin::Block::consensus_decode(&mut cursor)
        .map_err(|e| anyhow!("Failed to decode block: {}", e))
}

pub fn get_transaction(txid: &bitcoin::Txid) -> Result<bitcoin::Transaction> {
    let tx_data = TRANSACTIONS.select(&txid.to_byte_array().to_vec()).get();
    if tx_data.len() == 0 {
        return Err(anyhow!("Transaction not found for txid: {}", txid));
    }

    let mut cursor = Cursor::new(tx_data.as_ref().to_vec());
    bitcoin::Transaction::consensus_decode(&mut cursor)
        .map_err(|e| anyhow!("Failed to decode transaction: {}", e))
}
