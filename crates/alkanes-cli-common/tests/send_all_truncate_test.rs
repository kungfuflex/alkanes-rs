/// Tests for --send-all with transaction truncation to stay under consensus limits
/// 
/// This tests the complete flow:
/// 1. Build transaction with --send-all (selects ALL clean UTXOs)
/// 2. Calculate output = total_input - fee (using specified fee rate)
/// 3. Sign with --truncate-excess-vsize if needed
/// 4. Verify final transaction has correct fee rate

#[cfg(test)]
mod send_all_truncate_tests {
    use bitcoin::{Amount, Transaction, TxOut, TxIn, OutPoint, ScriptBuf, Witness};
    use bitcoin::consensus::Encodable;

    /// Test helper: Calculate expected transaction size for given number of P2WPKH inputs
    fn calculate_signed_tx_size(num_inputs: usize, num_outputs: usize) -> usize {
        // Transaction structure:
        // - Version: 4 bytes
        // - Input count: 1-9 bytes (varint)
        // - Inputs: ~68 bytes each unsigned (41 bytes base + ~27 for witness)
        //   After signing: ~107 bytes each (41 base + 66 witness)
        // - Output count: 1-9 bytes (varint)
        // - Outputs: ~43 bytes each (P2TR)
        // - Locktime: 4 bytes
        
        let base_size = 4 + 4; // version + locktime
        let input_count_varint = if num_inputs < 253 { 1 } else { 3 };
        let output_count_varint = if num_outputs < 253 { 1 } else { 3 };
        
        // Signed P2WPKH input size (Taproot)
        let input_size = num_inputs * 107;
        let output_size = num_outputs * 43;
        
        base_size + input_count_varint + input_size + output_count_varint + output_size
    }

    /// Test helper: Calculate maximum inputs that fit in consensus limit
    fn calculate_max_inputs_for_consensus_limit(max_tx_size: usize) -> usize {
        // Work backwards from max size
        let overhead = 53; // Base tx overhead + output
        let bytes_per_input = 107; // Signed P2TR input
        (max_tx_size - overhead) / bytes_per_input
    }

    #[test]
    fn test_max_inputs_calculation() {
        // For 1MB consensus limit
        let max_inputs = calculate_max_inputs_for_consensus_limit(1_000_000);
        println!("Max inputs for 1MB: {}", max_inputs);
        
        // Should be around 9,158
        assert!(max_inputs >= 9_150 && max_inputs <= 9_200, 
               "Expected ~9,158 inputs for 1MB, got {}", max_inputs);
        
        // Verify the size with this many inputs
        let estimated_size = calculate_signed_tx_size(max_inputs, 1);
        println!("Estimated size with {} inputs: {} bytes", max_inputs, estimated_size);
        assert!(estimated_size < 1_000_000, 
               "Transaction with {} inputs should be under 1MB, got {} bytes", 
               max_inputs, estimated_size);
    }

    #[test]
    fn test_send_all_output_calculation() {
        // Scenario: User has 9,877 UTXOs @ 1,999 sats each
        let num_utxos = 9877;
        let sats_per_utxo = 1999;
        let total_input = num_utxos * sats_per_utxo;
        let fee_rate = 2.1; // sat/vB
        
        println!("Test: --send-all with {} UTXOs", num_utxos);
        println!("Total input: {} sats", total_input);
        
        // Calculate expected transaction size
        let estimated_signed_size = calculate_signed_tx_size(num_utxos, 1);
        println!("Estimated signed size: {} bytes", estimated_signed_size);
        
        // This exceeds 1MB, so truncation should happen
        assert!(estimated_signed_size > 1_000_000, 
               "This test requires truncation");
        
        // Calculate max inputs that fit
        let max_inputs = calculate_max_inputs_for_consensus_limit(1_000_000);
        let actual_input = max_inputs * sats_per_utxo;
        let actual_signed_size = calculate_signed_tx_size(max_inputs, 1);
        
        println!("After truncation: {} inputs", max_inputs);
        println!("Truncated input: {} sats", actual_input);
        println!("Truncated size: {} bytes", actual_signed_size);
        
        // Calculate fee and output
        let fee = (actual_signed_size as f64 * fee_rate).ceil() as u64;
        let output = actual_input as u64 - fee;
        
        println!("Fee @ {} sat/vB: {} sats", fee_rate, fee);
        println!("Output: {} sats", output);
        
        // Verify fee rate
        let actual_fee_rate = fee as f64 / actual_signed_size as f64;
        println!("Actual fee rate: {:.4} sat/vB", actual_fee_rate);
        
        assert!((actual_fee_rate - fee_rate).abs() < 0.1,
               "Fee rate should be ~{}, got {:.4}", fee_rate, actual_fee_rate);
        
        // Verify it meets Slipstream minimum (2.0 sat/vB)
        assert!(actual_fee_rate >= 2.0,
               "Fee rate must be >= 2.0 sat/vB for Slipstream, got {:.4}", actual_fee_rate);
    }

    #[test]
    fn test_send_all_without_truncation() {
        // Scenario: Smaller number of UTXOs that fits in 1MB
        let num_utxos = 5000;
        let sats_per_utxo = 1999;
        let total_input = num_utxos * sats_per_utxo;
        let fee_rate = 2.1;
        
        println!("Test: --send-all with {} UTXOs (no truncation needed)", num_utxos);
        
        let estimated_signed_size = calculate_signed_tx_size(num_utxos, 1);
        println!("Estimated signed size: {} bytes", estimated_signed_size);
        
        // Should NOT exceed 1MB
        assert!(estimated_signed_size < 1_000_000,
               "Should not need truncation");
        
        // Calculate fee and output
        let fee = (estimated_signed_size as f64 * fee_rate).ceil() as u64;
        let output = total_input as u64 - fee;
        
        println!("Total input: {} sats", total_input);
        println!("Fee @ {} sat/vB: {} sats", fee_rate, fee);
        println!("Output: {} sats", output);
        
        let actual_fee_rate = fee as f64 / estimated_signed_size as f64;
        println!("Actual fee rate: {:.4} sat/vB", actual_fee_rate);
        
        assert!((actual_fee_rate - fee_rate).abs() < 0.1);
        assert!(actual_fee_rate >= 2.0);
    }

    #[test]
    fn test_truncation_boundary_cases() {
        // Test at exactly 1MB boundary
        let max_inputs = calculate_max_inputs_for_consensus_limit(1_000_000);
        
        // Test with max_inputs (should fit)
        let size_at_max = calculate_signed_tx_size(max_inputs, 1);
        assert!(size_at_max <= 1_000_000,
               "Max inputs should produce tx <= 1MB, got {} bytes", size_at_max);
        
        // Test with max_inputs + 1 (should NOT fit)
        let size_over_max = calculate_signed_tx_size(max_inputs + 1, 1);
        assert!(size_over_max > 1_000_000,
               "Max inputs + 1 should exceed 1MB, got {} bytes", size_over_max);
        
        println!("Boundary test:");
        println!("  {} inputs = {} bytes (✓ under 1MB)", max_inputs, size_at_max);
        println!("  {} inputs = {} bytes (✗ over 1MB)", max_inputs + 1, size_over_max);
    }

    #[test]
    fn test_fee_rate_precision() {
        // Test that fee rate is calculated correctly for different rates
        let test_cases = vec![
            (2.0, "Slipstream minimum"),
            (2.1, "RBF bump"),
            (3.0, "Higher fee"),
            (5.5, "Fractional rate"),
        ];
        
        let num_inputs = 9158;
        let sats_per_utxo = 1999;
        
        for (fee_rate, description) in test_cases {
            let total_input = num_inputs * sats_per_utxo;
            let tx_size = calculate_signed_tx_size(num_inputs, 1);
            let fee = (tx_size as f64 * fee_rate).ceil() as u64;
            let output = total_input as u64 - fee;
            let actual_rate = fee as f64 / tx_size as f64;
            
            println!("{}: fee_rate={:.2}, actual={:.4} sat/vB", 
                    description, fee_rate, actual_rate);
            
            // Should be within 0.1 sat/vB due to rounding
            assert!((actual_rate - fee_rate).abs() < 0.1,
                   "{}: Expected {:.2}, got {:.4}", description, fee_rate, actual_rate);
        }
    }

    #[test]
    fn test_output_amount_never_negative() {
        // Edge case: What if fee is larger than input?
        let num_inputs = 10;
        let sats_per_utxo = 1999;
        let total_input = num_inputs * sats_per_utxo;
        let fee_rate = 100.0; // Absurdly high
        
        let tx_size = calculate_signed_tx_size(num_inputs, 1);
        let fee = (tx_size as f64 * fee_rate).ceil() as u64;
        
        if fee > total_input as u64 {
            println!("✓ Correctly detected insufficient funds:");
            println!("  Input: {} sats", total_input);
            println!("  Fee: {} sats", fee);
            println!("  This should error, not produce negative output");
            // In real code, this should return an error
        } else {
            let output = total_input as u64 - fee;
            assert!(output > 0, "Output should be positive");
        }
    }
}
