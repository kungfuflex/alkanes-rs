/// Test for accurate fee calculation with P2TR inputs
/// 
/// The issue: P2TR inputs have witness discount, so vSize ≠ raw size
/// 
/// P2TR Input breakdown:
/// - Non-witness data (full weight): 41 bytes
///   * Outpoint (txid + vout): 36 bytes
///   * Sequence: 4 bytes
///   * Script length: 1 byte (0x00 for P2TR)
/// - Witness data (25% weight): ~66 bytes
///   * Signature: 64 bytes (Schnorr)
///   * Witness count: 1-2 bytes
/// 
/// Weight calculation:
/// - Non-witness: 41 × 4 = 164 WU
/// - Witness: 66 × 1 = 66 WU
/// - Total per input: 230 WU
/// - vSize per input: 230 / 4 = 57.5 vbytes
/// 
/// NOT 68 vbytes as currently estimated!

#[cfg(test)]
mod fee_calculation_tests {
    use bitcoin::{Transaction, TxIn, TxOut, OutPoint, Txid, Sequence, Witness, Amount, ScriptBuf};
    use std::str::FromStr;

    /// Calculate the actual vSize of a transaction
    fn calculate_actual_vsize(tx: &Transaction) -> u64 {
        // vSize = (weight + 3) / 4
        // Weight = (base_size × 3) + total_size
        // OR: Weight = non_witness_size × 4 + witness_size
        tx.vsize() as u64
    }

    /// Calculate the weight of a transaction
    fn calculate_weight(tx: &Transaction) -> u64 {
        tx.weight().to_wu()
    }

    /// Old (incorrect) estimation
    fn estimate_tx_vsize_old(num_inputs: usize, num_outputs: usize) -> u64 {
        let base_vsize = 10;
        let input_vsize = 68; // WRONG for P2TR!
        let output_vsize = 43;
        base_vsize + (num_inputs as u64 * input_vsize) + (num_outputs as u64 * output_vsize)
    }

    /// New (correct) estimation for P2TR inputs
    fn estimate_tx_vsize_new(num_inputs: usize, num_outputs: usize) -> u64 {
        // Transaction overhead
        let base_overhead = 10; // version (4) + locktime (4) + input/output counts (~2)
        
        // P2TR input breakdown:
        // Non-witness data (full weight):
        //   - Outpoint: 36 bytes (txid + vout)
        //   - Sequence: 4 bytes
        //   - Script length: 1 byte (0x00)
        //   - Total non-witness: 41 bytes × 4 = 164 WU
        // Witness data (1x weight):
        //   - Witness count: 1 byte
        //   - Signature: 64 bytes (Schnorr)
        //   - Total witness: 65 bytes × 1 = 65 WU
        // Total per input: 164 + 65 = 229 WU
        // vSize per input: 229 / 4 = 57.25 vbytes
        let input_weight = 229; // WU per P2TR input
        let input_vsize = (input_weight + 3) / 4; // = 57.5 vbytes, rounds to 58
        
        // P2TR output:
        //   - Amount: 8 bytes
        //   - Script length: 1 byte
        //   - Script: 34 bytes (0x5120 + 32 byte pubkey)
        //   - Total: 43 bytes (no witness discount for outputs)
        let output_vsize = 43;
        
        // Witness flag overhead: 2 bytes (0x00 0x01) = 2 WU = 0.5 vbytes
        let witness_overhead_vsize = 1;
        
        base_overhead + 
        (num_inputs as u64 * input_vsize) + 
        (num_outputs as u64 * output_vsize) +
        witness_overhead_vsize
    }

    #[test]
    fn test_single_p2tr_input_vsize() {
        println!("\n=== Test: Single P2TR Input Transaction ===\n");
        
        // Create a minimal P2TR transaction with 1 input, 1 output
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        
        // Add 1 P2TR input with signature
        let dummy_outpoint = OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            vout: 0,
        };
        
        let mut witness = Witness::new();
        witness.push(&[0u8; 64]); // 64-byte Schnorr signature
        
        tx.input.push(TxIn {
            previous_output: dummy_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness,
        });
        
        // Add 1 P2TR output
        let p2tr_script = ScriptBuf::from_hex("51200000000000000000000000000000000000000000000000000000000000000000").unwrap();
        tx.output.push(TxOut {
            value: Amount::from_sat(100000),
            script_pubkey: p2tr_script,
        });
        
        let actual_vsize = calculate_actual_vsize(&tx);
        let actual_weight = calculate_weight(&tx);
        let old_estimate = estimate_tx_vsize_old(1, 1);
        let new_estimate = estimate_tx_vsize_new(1, 1);
        
        println!("Actual vSize:        {} vbytes", actual_vsize);
        println!("Actual weight:       {} WU", actual_weight);
        println!("Old estimate (68/i): {} vbytes (error: {} vbytes)", old_estimate, old_estimate as i64 - actual_vsize as i64);
        println!("New estimate (57/i): {} vbytes (error: {} vbytes)", new_estimate, new_estimate as i64 - actual_vsize as i64);
        
        // New estimate should be within 5 vbytes of actual
        assert!((new_estimate as i64 - actual_vsize as i64).abs() < 5,
                "New estimate {} should be close to actual {}", new_estimate, actual_vsize);
        
        println!("\n✅ New estimation is accurate!\n");
    }

    #[test]
    fn test_large_consolidation_vsize() {
        println!("\n=== Test: Large Consolidation (9,158 inputs) ===\n");
        
        let num_inputs = 9158;
        let num_outputs = 1;
        
        let old_estimate = estimate_tx_vsize_old(num_inputs, num_outputs);
        let new_estimate = estimate_tx_vsize_new(num_inputs, num_outputs);
        
        // Expected breakdown for 9,158 inputs, 1 output:
        // Base: 10 vbytes
        // Inputs: 9,158 × 57.5 = 526,585 vbytes
        // Outputs: 1 × 43 = 43 vbytes
        // Witness overhead: 1 vbyte
        // Total: ~526,639 vbytes
        
        println!("Number of inputs:  {}", num_inputs);
        println!("Number of outputs: {}", num_outputs);
        println!("");
        println!("Old estimate (68 vb/input): {:>10} vbytes", old_estimate);
        println!("New estimate (57 vb/input): {:>10} vbytes", new_estimate);
        println!("Difference:                 {:>10} vbytes", old_estimate - new_estimate);
        println!("");
        
        // At 1 sat/vB:
        let old_fee = old_estimate;
        let new_fee = new_estimate;
        let savings = old_fee - new_fee;
        
        println!("Fee impact at 1 sat/vB:");
        println!("  Old fee estimate: {} sats", old_fee);
        println!("  New fee estimate: {} sats", new_fee);
        println!("  Savings:          {} sats ({:.2}%)", savings, (savings as f64 / old_fee as f64) * 100.0);
        println!("");
        
        // For 18.3M sats input:
        let total_input = 18_306_842;
        let old_output = total_input - old_fee;
        let new_output = total_input - new_fee;
        let extra_received = new_output - old_output;
        
        println!("Output comparison (18.3M sats input):");
        println!("  Old output: {} sats", old_output);
        println!("  New output: {} sats", new_output);
        println!("  Extra received: {} sats", extra_received);
        println!("");
        
        // Verify the new estimate matches our actual transaction
        // Actual from decode: 526,641 vbytes
        let actual_vsize = 526_641;
        let error = (new_estimate as i64 - actual_vsize as i64).abs();
        
        println!("Comparison to actual transaction:");
        println!("  Actual vSize:    {} vbytes", actual_vsize);
        println!("  New estimate:    {} vbytes", new_estimate);
        println!("  Error:           {} vbytes ({:.3}%)", error, (error as f64 / actual_vsize as f64) * 100.0);
        println!("");
        
        assert!(error < 100, "Estimate should be within 100 vbytes of actual");
        
        println!("✅ New estimation is accurate for large consolidations!\n");
    }

    #[test]
    fn test_fee_rate_accuracy() {
        println!("\n=== Test: Fee Rate Accuracy ===\n");
        
        let num_inputs = 9158;
        let total_input = 18_306_842;
        let target_fee_rate = 1.0; // sat/vB
        
        // Using new accurate estimation
        let estimated_vsize = estimate_tx_vsize_new(num_inputs, 1);
        let calculated_fee = (estimated_vsize as f64 * target_fee_rate).ceil() as u64;
        let output_amount = total_input - calculated_fee;
        
        println!("Target fee rate:   {} sat/vB", target_fee_rate);
        println!("Estimated vSize:   {} vbytes", estimated_vsize);
        println!("Calculated fee:    {} sats", calculated_fee);
        println!("Output amount:     {} sats", output_amount);
        println!("");
        
        // Verify against actual transaction
        let actual_vsize = 526_641;
        let actual_fee_for_target = (actual_vsize as f64 * target_fee_rate).ceil() as u64;
        
        println!("Actual transaction:");
        println!("  Actual vSize:    {} vbytes", actual_vsize);
        println!("  Fee for 1 sat/vB: {} sats", actual_fee_for_target);
        println!("");
        
        let fee_error = (calculated_fee as i64 - actual_fee_for_target as i64).abs();
        let fee_error_percent = (fee_error as f64 / actual_fee_for_target as f64) * 100.0;
        
        println!("Accuracy:");
        println!("  Fee error:       {} sats ({:.3}%)", fee_error, fee_error_percent);
        println!("");
        
        // Should be within 0.1% error
        assert!(fee_error_percent < 0.1, "Fee calculation should be within 0.1% of actual");
        
        println!("✅ Fee rate calculation is accurate!\n");
    }

    #[test]
    fn test_current_transaction_analysis() {
        println!("\n=== Analysis of Current Transaction ===\n");
        
        // Current transaction stats
        let actual_vsize = 526_641;
        let actual_output = 16_681_636;
        let total_input = 18_306_842;
        let actual_fee = total_input - actual_output;
        let actual_fee_rate = actual_fee as f64 / actual_vsize as f64;
        
        println!("Current transaction:");
        println!("  Total input:     {} sats", total_input);
        println!("  Output:          {} sats", actual_output);
        println!("  Fee:             {} sats", actual_fee);
        println!("  vSize:           {} vbytes", actual_vsize);
        println!("  Fee rate:        {:.2} sat/vB", actual_fee_rate);
        println!("");
        
        // What we should have with correct calculation
        let correct_fee_1satvb = actual_vsize;
        let correct_output_1satvb = total_input - correct_fee_1satvb;
        let overpaid = actual_fee - correct_fee_1satvb;
        
        println!("With correct 1 sat/vB calculation:");
        println!("  Fee:             {} sats", correct_fee_1satvb);
        println!("  Output:          {} sats", correct_output_1satvb);
        println!("  Overpaid by:     {} sats", overpaid);
        println!("");
        
        println!("Cause of overpayment:");
        println!("  Old estimate used 68 vbytes/input");
        println!("  Should use ~57.5 vbytes/input");
        println!("  Error per input: ~10.5 vbytes");
        println!("  Total error:     {} vbytes", 9158 * 10);
        println!("");
        
        println!("✅ Analysis confirms fee calculation needs fixing!\n");
    }
}
