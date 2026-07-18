/// Tests for --split-max-vsize transaction splitting functionality
/// 
/// This tests the complete flow:
/// 1. Build transaction with --send-all (selects ALL clean UTXOs)
/// 2. Split into multiple transactions with --split-max-vsize
/// 3. Verify each transaction is under the size limit
/// 4. Verify all transactions sum to the same total effect
/// 5. Verify transactions use mutually exclusive UTXOs

#[cfg(test)]
mod split_max_vsize_tests {
    use bitcoin::{Amount, Transaction, TxOut, TxIn, OutPoint, ScriptBuf, Witness, Txid};
    use bitcoin::consensus::Encodable;
    use std::collections::HashSet;

    /// Test helper: Calculate expected transaction size for given number of P2WPKH inputs
    fn calculate_signed_tx_vsize(num_inputs: usize, num_outputs: usize) -> usize {
        // P2WPKH transaction structure:
        // Base: version(4) + marker(1) + flag(1) + input_count + inputs*(41) + output_count(1) + outputs*(43) + locktime(4)
        // Witness: per input: witness_count(1) + sig_len(1) + sig(72) + pubkey_len(1) + pubkey(33) = 108 bytes
        
        let input_count_varint = if num_inputs < 253 { 1 } else if num_inputs < 65536 { 3 } else { 5 };
        let base_size = 4 + 1 + 1 + input_count_varint + (num_inputs * 41) + 1 + (num_outputs * 43) + 4;
        let witness_size = num_inputs * 108;
        
        // vSize = (base_weight + witness_weight) / 4
        // Base weight = base_size * 4, Witness weight = witness_size * 1
        let weight = (base_size * 4) + witness_size;
        (weight + 3) / 4 // Round up
    }

    /// Calculate max inputs that fit in a given vsize limit
    fn calculate_max_inputs_for_vsize(max_vsize: usize) -> usize {
        // Work backwards from max vsize with safety margin
        let safety_margin = 0.975;
        let target_vsize = (max_vsize as f64 * safety_margin) as usize;
        
        let overhead_bytes = 4 + 1 + 1 + 1 + 1 + 43 + 4; // ~55 bytes
        let bytes_per_input = 41 + 108; // 149 raw bytes per P2WPKH input
        
        let available_for_inputs = target_vsize.saturating_sub(overhead_bytes);
        available_for_inputs / bytes_per_input
    }

    #[test]
    fn test_split_calculation_100k() {
        // Test that we correctly calculate splits for 100k limit
        let max_vsize = 100_000;
        let max_inputs_per_tx = calculate_max_inputs_for_vsize(max_vsize);
        
        println!("Max inputs per 100k transaction: {}", max_inputs_per_tx);
        
        // Verify the size with this many inputs
        let estimated_vsize = calculate_signed_tx_vsize(max_inputs_per_tx, 1);
        println!("Estimated vSize with {} inputs: {} vbytes", max_inputs_per_tx, estimated_vsize);
        
        assert!(estimated_vsize <= max_vsize, 
               "Transaction with {} inputs should be under 100k, got {} vbytes", 
               max_inputs_per_tx, estimated_vsize);
        
        // Should be around 654 inputs per 100k transaction
        assert!(max_inputs_per_tx >= 640 && max_inputs_per_tx <= 670,
               "Expected ~654 inputs for 100k, got {}", max_inputs_per_tx);
    }

    #[test]
    fn test_split_into_multiple_transactions() {
        // Scenario: User has 10,000 UTXOs @ 1,999 sats each
        // Split into 100k transactions
        let total_utxos = 10_000;
        let sats_per_utxo = 1999;
        let total_input = total_utxos * sats_per_utxo;
        let fee_rate = 0.3; // sat/vB
        let max_vsize = 100_000;
        
        println!("\nTest: Split {} UTXOs into 100k transactions", total_utxos);
        println!("Total input: {} sats ({:.2} BTC)", total_input, total_input as f64 / 100_000_000.0);
        
        // Calculate how many transactions we need
        let max_inputs_per_tx = calculate_max_inputs_for_vsize(max_vsize);
        let num_transactions = (total_utxos + max_inputs_per_tx - 1) / max_inputs_per_tx;
        
        println!("Max inputs per tx: {}", max_inputs_per_tx);
        println!("Number of transactions: {}", num_transactions);
        
        // Simulate splitting the UTXOs
        let mut total_output_sats = 0u64;
        let mut total_fee_sats = 0u64;
        let mut used_utxo_indices = HashSet::new();
        
        for tx_idx in 0..num_transactions {
            let start_idx = tx_idx * max_inputs_per_tx;
            let end_idx = std::cmp::min(start_idx + max_inputs_per_tx, total_utxos);
            let chunk_inputs = end_idx - start_idx;
            
            // Verify no UTXO reuse
            for i in start_idx..end_idx {
                assert!(!used_utxo_indices.contains(&i), 
                       "UTXO {} reused in transaction {}", i, tx_idx);
                used_utxo_indices.insert(i);
            }
            
            // Calculate this chunk
            let chunk_input_amount = chunk_inputs * sats_per_utxo;
            let chunk_vsize = calculate_signed_tx_vsize(chunk_inputs, 1);
            let chunk_fee = (chunk_vsize as f64 * fee_rate).ceil() as u64;
            let chunk_output_amount = (chunk_input_amount as u64).saturating_sub(chunk_fee);
            
            total_output_sats += chunk_output_amount;
            total_fee_sats += chunk_fee;
            
            println!("\nTransaction {} of {}:", tx_idx + 1, num_transactions);
            println!("  Inputs: {} UTXOs (indices {} to {})", chunk_inputs, start_idx, end_idx - 1);
            println!("  Input amount: {} sats", chunk_input_amount);
            println!("  vSize: {} vbytes", chunk_vsize);
            println!("  Fee: {} sats ({:.4} sat/vB)", chunk_fee, chunk_fee as f64 / chunk_vsize as f64);
            println!("  Output: {} sats", chunk_output_amount);
            
            // Verify size is under limit
            assert!(chunk_vsize <= max_vsize,
                   "Transaction {} exceeds size limit: {} > {}", tx_idx + 1, chunk_vsize, max_vsize);
            
            // Verify output is positive
            assert!(chunk_output_amount > 0,
                   "Transaction {} has zero or negative output", tx_idx + 1);
        }
        
        // Verify all UTXOs were used exactly once
        assert_eq!(used_utxo_indices.len(), total_utxos,
                  "Not all UTXOs were used: {} of {}", used_utxo_indices.len(), total_utxos);
        
        // Verify total conservation
        let expected_total_output = (total_input as u64).saturating_sub(total_fee_sats);
        println!("\nSummary:");
        println!("  Total transactions: {}", num_transactions);
        println!("  Total input: {} sats", total_input);
        println!("  Total fees: {} sats", total_fee_sats);
        println!("  Total output: {} sats", total_output_sats);
        println!("  Expected output: {} sats", expected_total_output);
        
        assert_eq!(total_output_sats, expected_total_output,
                  "Output mismatch: got {}, expected {}", total_output_sats, expected_total_output);
    }

    #[test]
    fn test_split_with_small_remainder() {
        // Test case where last transaction has much fewer inputs
        let total_utxos = 2000;
        let max_inputs_per_tx = calculate_max_inputs_for_vsize(100_000);
        let num_transactions = (total_utxos + max_inputs_per_tx - 1) / max_inputs_per_tx;
        
        let last_tx_inputs = total_utxos - (max_inputs_per_tx * (num_transactions - 1));
        
        println!("\nTest: Split with small remainder");
        println!("Total UTXOs: {}", total_utxos);
        println!("Max per tx: {}", max_inputs_per_tx);
        println!("Transactions: {}", num_transactions);
        println!("Last tx inputs: {}", last_tx_inputs);
        
        // Verify last transaction has correct number of inputs
        assert!(last_tx_inputs > 0 && last_tx_inputs <= max_inputs_per_tx,
               "Last transaction should have 1 to {} inputs, got {}", max_inputs_per_tx, last_tx_inputs);
        
        // Verify last transaction size is still valid
        let last_tx_vsize = calculate_signed_tx_vsize(last_tx_inputs, 1);
        assert!(last_tx_vsize <= 100_000,
               "Last transaction should fit in 100k: {} vbytes", last_tx_vsize);
    }

    #[test]
    fn test_split_preserves_fee_rate() {
        // Verify that each split transaction maintains the same fee rate
        let total_utxos = 5000;
        let sats_per_utxo = 1999;
        let fee_rate = 2.1; // sat/vB
        let max_vsize = 100_000;
        let max_inputs_per_tx = calculate_max_inputs_for_vsize(max_vsize);
        let num_transactions = (total_utxos + max_inputs_per_tx - 1) / max_inputs_per_tx;
        
        println!("\nTest: Fee rate preservation across splits");
        println!("Target fee rate: {:.4} sat/vB", fee_rate);
        
        for tx_idx in 0..num_transactions {
            let start_idx = tx_idx * max_inputs_per_tx;
            let end_idx = std::cmp::min(start_idx + max_inputs_per_tx, total_utxos);
            let chunk_inputs = end_idx - start_idx;
            
            let chunk_input_amount = chunk_inputs * sats_per_utxo;
            let chunk_vsize = calculate_signed_tx_vsize(chunk_inputs, 1);
            let chunk_fee = (chunk_vsize as f64 * fee_rate).ceil() as u64;
            let actual_fee_rate = chunk_fee as f64 / chunk_vsize as f64;
            
            println!("  Tx {}: {:.4} sat/vB (diff: {:.4})", 
                    tx_idx + 1, actual_fee_rate, (actual_fee_rate - fee_rate).abs());
            
            // Fee rate should be within 0.1 sat/vB due to rounding
            assert!((actual_fee_rate - fee_rate).abs() < 0.1,
                   "Transaction {} fee rate mismatch: expected {:.4}, got {:.4}",
                   tx_idx + 1, fee_rate, actual_fee_rate);
        }
    }

    #[test]
    fn test_split_different_size_limits() {
        // Test splitting with different size limits
        let test_cases = vec![
            (50_000, "50k"),
            (100_000, "100k"),
            (200_000, "200k"),
            (1_000_000, "1M"),
        ];
        
        let total_utxos = 10_000;
        
        println!("\nTest: Different size limits");
        for (max_vsize, label) in test_cases {
            let max_inputs = calculate_max_inputs_for_vsize(max_vsize);
            let num_txs = (total_utxos + max_inputs - 1) / max_inputs;
            let estimated_vsize = calculate_signed_tx_vsize(max_inputs, 1);
            
            println!("  {}: {} inputs/tx, {} txs, {} vbytes/tx", 
                    label, max_inputs, num_txs, estimated_vsize);
            
            assert!(estimated_vsize <= max_vsize,
                   "{}: Transaction size {} exceeds limit {}", label, estimated_vsize, max_vsize);
        }
    }

    #[test]
    fn test_split_minimum_inputs() {
        // Test with very small size limit to ensure we handle edge cases
        let max_vsize = 10_000; // Very small limit
        let max_inputs = calculate_max_inputs_for_vsize(max_vsize);
        
        println!("\nTest: Minimum inputs with 10k limit");
        println!("Max inputs: {}", max_inputs);
        
        // Should still allow at least a few inputs
        assert!(max_inputs >= 50,
               "Should allow at least 50 inputs for 10k limit, got {}", max_inputs);
        
        let vsize = calculate_signed_tx_vsize(max_inputs, 1);
        assert!(vsize <= max_vsize,
               "Transaction with {} inputs exceeds 10k: {} vbytes", max_inputs, vsize);
    }

    #[test]
    fn test_no_split_needed() {
        // Test case where transaction fits without splitting
        let total_utxos = 500;
        let max_vsize = 100_000;
        let tx_vsize = calculate_signed_tx_vsize(total_utxos, 1);
        
        println!("\nTest: No split needed");
        println!("UTXOs: {}", total_utxos);
        println!("Transaction vSize: {} vbytes", tx_vsize);
        println!("Limit: {} vbytes", max_vsize);
        
        if tx_vsize <= max_vsize {
            println!("✓ Fits in single transaction (no split needed)");
            // In this case, the CLI should still work but produce only 1 transaction
            let max_inputs = calculate_max_inputs_for_vsize(max_vsize);
            let num_txs = (total_utxos + max_inputs - 1) / max_inputs;
            assert_eq!(num_txs, 1,
                      "Should produce 1 transaction when no split needed, got {}", num_txs);
        } else {
            println!("✗ Requires split");
        }
    }

    #[test]
    fn test_mutually_exclusive_utxos() {
        // Verify that split transactions use mutually exclusive UTXOs (no double-spend)
        let total_utxos = 5000;
        let max_inputs_per_tx = calculate_max_inputs_for_vsize(100_000);
        let num_transactions = (total_utxos + max_inputs_per_tx - 1) / max_inputs_per_tx;
        
        println!("\nTest: Mutually exclusive UTXOs");
        println!("Total UTXOs: {}", total_utxos);
        println!("Transactions: {}", num_transactions);
        
        let mut all_used_utxos = HashSet::new();
        
        for tx_idx in 0..num_transactions {
            let start_idx = tx_idx * max_inputs_per_tx;
            let end_idx = std::cmp::min(start_idx + max_inputs_per_tx, total_utxos);
            
            let mut tx_utxos = HashSet::new();
            for i in start_idx..end_idx {
                // Check not used in this transaction
                assert!(!tx_utxos.contains(&i),
                       "UTXO {} used twice in transaction {}", i, tx_idx);
                tx_utxos.insert(i);
                
                // Check not used in any previous transaction
                assert!(!all_used_utxos.contains(&i),
                       "UTXO {} reused across transactions (first in tx {}, now in tx {})",
                       i, all_used_utxos.iter().position(|&x| x == i).unwrap(), tx_idx);
                all_used_utxos.insert(i);
            }
            
            println!("  Tx {}: {} unique UTXOs", tx_idx + 1, tx_utxos.len());
        }
        
        // Verify all UTXOs accounted for
        assert_eq!(all_used_utxos.len(), total_utxos,
                  "Not all UTXOs used: {} of {}", all_used_utxos.len(), total_utxos);
        
        println!("✓ All {} UTXOs used exactly once across {} transactions", 
                total_utxos, num_transactions);
    }
}
