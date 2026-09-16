//! Unit tests for BlockLike and TransactionLike trait implementations on Zcash types
//!
//! These tests verify that Zcash blocks and transactions work correctly with the abstractions

#[cfg(all(test, feature = "zcash"))]
mod tests {
    use crate::block_traits::{BlockLike, TransactionLike};
    use crate::zcash::{ZcashBlock, ZcashTransaction};
    use bitcoin::hashes::Hash as HashTrait;
    use bitcoin::{BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxMerkleNode, TxOut, Txid};
    use std::io::Cursor;

    /// Raw hex of Zcash block 0 (genesis)
    const BLOCK_0_HEX: &str = include_str!("../../alkanes/src/tests/blocks/zec_0.hex");

    /// Raw hex of Zcash block 250
    const BLOCK_250_HEX: &str = include_str!("../../alkanes/src/tests/blocks/zec_250.hex");

    #[test]
    fn test_zcash_transaction_like_basic() {
        let block_bytes = hex::decode(BLOCK_0_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        assert!(zblock.transactions.len() > 0, "Block should have transactions");

        let tx = &zblock.transactions[0];
        
        // Test TransactionLike methods
        assert!(tx.inputs().len() > 0, "Should have inputs");
        assert!(tx.outputs().len() > 0, "Should have outputs");
        assert!(tx.is_coinbase(), "First transaction should be coinbase");
        
        let txid = tx.txid();
        assert_ne!(txid, Txid::all_zeros(), "TXID should not be all zeros");
    }

    #[test]
    fn test_zcash_transaction_to_bitcoin_tx() {
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let ztx = &zblock.transactions[0];
        let btc_tx = ztx.to_bitcoin_tx();

        // Verify conversion preserves transparent parts
        assert_eq!(
            btc_tx.input.len(),
            ztx.inputs().len(),
            "Should preserve input count"
        );
        assert_eq!(
            btc_tx.output.len(),
            ztx.outputs().len(),
            "Should preserve output count"
        );
        assert_eq!(
            btc_tx.version.0, ztx.version,
            "Should preserve version"
        );

        // Verify inputs match
        for (i, inp) in btc_tx.input.iter().enumerate() {
            assert_eq!(
                inp.previous_output,
                ztx.inputs()[i].previous_output,
                "Input {} should match",
                i
            );
        }

        // Verify outputs match
        for (i, out) in btc_tx.output.iter().enumerate() {
            assert_eq!(
                out.value,
                ztx.outputs()[i].value,
                "Output {} value should match",
                i
            );
            assert_eq!(
                out.script_pubkey,
                ztx.outputs()[i].script_pubkey,
                "Output {} script should match",
                i
            );
        }
    }

    #[test]
    fn test_zcash_block_like_basic() {
        let block_bytes = hex::decode(BLOCK_0_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        // Test BlockLike methods
        let hash = zblock.block_hash();
        assert_ne!(hash, BlockHash::all_zeros(), "Block hash should not be all zeros");

        let txs = zblock.transactions();
        assert!(txs.len() > 0, "Should have transactions");

        let header = zblock.header();
        assert_eq!(header.version.to_consensus(), zblock.version, "Version should match");
    }

    #[test]
    fn test_zcash_block_to_bitcoin_block() {
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let btc_block = zblock.to_bitcoin_block();

        // Verify conversion preserves structure
        assert_eq!(
            btc_block.txdata.len(),
            zblock.transactions().len(),
            "Should preserve transaction count"
        );

        // Verify header is converted correctly
        assert_eq!(
            btc_block.header.time,
            zblock.time,
            "Header time should match"
        );

        // Verify each transaction is converted
        for (i, tx) in btc_block.txdata.iter().enumerate() {
            let ztx = &zblock.transactions[i];
            assert_eq!(
                tx.input.len(),
                ztx.inputs().len(),
                "Transaction {} input count should match",
                i
            );
            assert_eq!(
                tx.output.len(),
                ztx.outputs().len(),
                "Transaction {} output count should match",
                i
            );
        }
    }

    #[test]
    fn test_zcash_block_to_bitcoin_block_preserves_vfsize() {
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let btc_block = zblock.to_bitcoin_block();

        // Calculate vfsize
        let vfsize: u64 = btc_block
            .txdata
            .iter()
            .map(|tx| {
                use bitcoin::consensus::Encodable;
                let mut buf = Vec::new();
                tx.consensus_encode(&mut buf).unwrap();
                buf.len() as u64
            })
            .sum();

        println!("Zcash block 250 converted vfsize: {}", vfsize);
        assert!(vfsize > 0, "vfsize should be non-zero (got {})", vfsize);
        assert!(vfsize < 10000000, "vfsize should be reasonable (got {})", vfsize);
    }

    #[test]
    fn test_zcash_block_vfsize_non_zero() {
        // This is the CRITICAL test - ensuring vfsize is never 0 after conversion
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let btc_block = zblock.to_bitcoin_block();

        assert_eq!(
            btc_block.txdata.len(),
            1,
            "Block 250 should have 1 transaction"
        );
        assert!(
            !btc_block.txdata.is_empty(),
            "Converted block should not be empty"
        );

        // This is what FuelTank::initialize calls
        let vfsize: u64 = btc_block
            .txdata
            .iter()
            .map(|tx| {
                use bitcoin::consensus::Encodable;
                let mut buf = Vec::new();
                tx.consensus_encode(&mut buf).unwrap();
                buf.len() as u64
            })
            .sum();

        assert_ne!(
            vfsize, 0,
            "CRITICAL: vfsize must not be zero (this causes division by zero in FuelTank)"
        );
    }

    #[test]
    fn test_zcash_transparent_output_preservation() {
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let btc_block = zblock.to_bitcoin_block();
        let ztx = &zblock.transactions[0];
        let btc_tx = &btc_block.txdata[0];

        // Verify all transparent outputs are preserved
        assert_eq!(
            btc_tx.output.len(),
            ztx.outputs().len(),
            "All outputs should be preserved"
        );

        for (i, out) in btc_tx.output.iter().enumerate() {
            let zout = &ztx.outputs()[i];
            assert_eq!(
                out.value, zout.value,
                "Output {} value should match",
                i
            );
            assert_eq!(
                out.script_pubkey, zout.script_pubkey,
                "Output {} script_pubkey should match",
                i
            );
        }
    }

    #[test]
    fn test_zcash_coinbase_detection() {
        let block_bytes = hex::decode(BLOCK_0_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        let tx = &zblock.transactions[0];
        assert!(
            tx.is_coinbase(),
            "First transaction in block should be coinbase"
        );

        // Verify coinbase detection works through trait
        let btc_tx = tx.to_bitcoin_tx();
        assert!(
            btc_tx.is_coinbase(),
            "Converted transaction should still be detected as coinbase"
        );
    }

    #[test]
    fn test_zcash_txid_consistency() {
        let block_bytes = hex::decode(BLOCK_250_HEX.trim()).expect("Failed to decode hex");
        let mut cursor = Cursor::new(block_bytes);
        let zblock = ZcashBlock::parse(&mut cursor).expect("Failed to parse block");

        for (i, ztx) in zblock.transactions.iter().enumerate() {
            let txid1 = ztx.txid();
            let txid2 = ztx.compute_txid();
            assert_eq!(
                txid1, txid2,
                "Transaction {} txid() should match compute_txid()",
                i
            );

            // Verify converted transaction has correct txid
            let btc_tx = ztx.to_bitcoin_tx();
            let btc_txid = btc_tx.compute_txid();
            // Note: txids might differ because Bitcoin TX doesn't include shielded components
            // But we're checking the method works without panicking
            assert_ne!(btc_txid, Txid::all_zeros(), "Converted txid should be valid");
        }
    }
}
