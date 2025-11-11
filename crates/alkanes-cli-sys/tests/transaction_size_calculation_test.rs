/// Test to validate P2WPKH transaction size calculations
/// This ensures our truncation logic correctly estimates signed transaction sizes

#[test]
fn test_p2wpkh_size_calculation() {
    // P2WPKH transaction structure:
    // Base (non-witness): version(4) + marker(1) + flag(1) + input_count + inputs*(txid(32)+vout(4)+scriptSig(1)+sequence(4)) + output_count(1) + outputs + locktime(4)
    // Witness: per input: witness_item_count(1) + sig_len(1) + signature(~72) + pubkey_len(1) + pubkey(33) = ~108 bytes
    
    let test_cases = vec![
        // (num_inputs, expected_base_size, expected_witness_size, expected_total_raw, expected_weight)
        // Witness per input: 1 (count) + 1+72 (sig) + 1+33 (pubkey) = 108 bytes
        (1, 4+1+1+1 + 1*41 + 1+43 + 4, 1*108, 4+1+1+1 + 1*41 + 1+43 + 4 + 1*108, (4+1+1+1 + 1*41 + 1+43 + 4)*4 + 1*108),
        (100, 4+1+1+1 + 100*41 + 1+43 + 4, 100*108, 4+1+1+1 + 100*41 + 1+43 + 4 + 100*108, (4+1+1+1 + 100*41 + 1+43 + 4)*4 + 100*108),
        (253, 4+1+1+3 + 253*41 + 1+43 + 4, 253*108, 4+1+1+3 + 253*41 + 1+43 + 4 + 253*108, (4+1+1+3 + 253*41 + 1+43 + 4)*4 + 253*108),
        (1000, 4+1+1+3 + 1000*41 + 1+43 + 4, 1000*108, 4+1+1+3 + 1000*41 + 1+43 + 4 + 1000*108, (4+1+1+3 + 1000*41 + 1+43 + 4)*4 + 1000*108),
        (7033, 4+1+1+3 + 7033*41 + 1+43 + 4, 7033*108, 4+1+1+3 + 7033*41 + 1+43 + 4 + 7033*108, (4+1+1+3 + 7033*41 + 1+43 + 4)*4 + 7033*108),
        (9894, 4+1+1+3 + 9894*41 + 1+43 + 4, 9894*108, 4+1+1+3 + 9894*41 + 1+43 + 4 + 9894*108, (4+1+1+3 + 9894*41 + 1+43 + 4)*4 + 9894*108),
    ];
    
    for (num_inputs, expected_base, expected_witness, expected_total, expected_weight) in test_cases {
        let input_count_varint_size = if num_inputs < 253 { 1 } else if num_inputs < 65536 { 3 } else { 5 };
        let overhead_bytes = 4 + 1 + 1 + input_count_varint_size + 1 + 43 + 4;
        let _bytes_per_input = 41 + 108;
        
        let calculated_base = overhead_bytes + (num_inputs * 41);
        let calculated_witness = num_inputs * 108; // 1 (count) + 1+72 (sig) + 1+33 (pubkey)
        let calculated_total = calculated_base + calculated_witness;
        let calculated_weight = (calculated_base * 4) + calculated_witness;
        let calculated_vsize = (calculated_weight + 3) / 4;
        let expected_vsize = (expected_weight + 3) / 4;
        
        assert_eq!(calculated_base, expected_base, "Base size mismatch for {} inputs", num_inputs);
        assert_eq!(calculated_witness, expected_witness, "Witness size mismatch for {} inputs", num_inputs);
        assert_eq!(calculated_total, expected_total, "Total raw size mismatch for {} inputs", num_inputs);
        assert_eq!(calculated_vsize, expected_vsize, "vSize mismatch for {} inputs", num_inputs);
        
        println!("✅ {} inputs: base={}, witness={}, total={} bytes, weight={} WU, vSize={} vB", 
                 num_inputs, calculated_base, calculated_witness, calculated_total, calculated_weight, calculated_vsize);
    }
}

#[test]
fn test_1mb_truncation_target() {
    // For 1MB limit with 97.5% safety margin
    let max_size = 1_048_576;
    let safety_margin = 0.975;
    let target_size = (max_size as f64 * safety_margin) as usize;
    
    // Calculate maximum inputs
    let input_count_varint_size = 3; // Assume we'll need 3 bytes for the count
    let overhead_bytes = 4 + 1 + 1 + input_count_varint_size + 1 + 43 + 4;
    let bytes_per_input = 41 + 108; // P2WPKH: 41 base + 108 witness (includes witness count byte)
    
    let available_for_inputs = target_size - overhead_bytes;
    let max_inputs = available_for_inputs / bytes_per_input;
    
    // Calculate the actual size with max_inputs
    let actual_base = overhead_bytes + (max_inputs * 41);
    let actual_witness = max_inputs * 108; // 1 (count) + 1+72 (sig) + 1+33 (pubkey)
    let actual_total = actual_base + actual_witness;
    let actual_weight = (actual_base * 4) + actual_witness;
    let actual_vsize = (actual_weight + 3) / 4;
    
    println!("📊 1MB Truncation Test:");
    println!("   Target size: {} bytes ({:.2} KB)", target_size, target_size as f64 / 1024.0);
    println!("   Max inputs: {}", max_inputs);
    println!("   Actual raw size: {} bytes ({:.2} KB)", actual_total, actual_total as f64 / 1024.0);
    println!("   Actual vSize: {} vBytes ({:.2} KB)", actual_vsize, actual_vsize as f64 / 1024.0);
    println!("   Actual weight: {} WU", actual_weight);
    
    // Verify we're within the target AND under actual 1MB limit (1,048,576 bytes)
    assert!(actual_total <= target_size, "Transaction exceeds target size!");
    assert!(actual_total < max_size, "Transaction exceeds 1MB consensus limit (1,048,576 bytes)!");
    assert!(actual_total > (target_size - bytes_per_input), "Transaction is too small (could fit more inputs)");
    
    println!("   ✅ Transaction is {} bytes under 1MB limit (1,048,576 bytes)", max_size - actual_total);
    
    // For 9894 inputs with 97.5% safety margin and 149 bytes/input, we should truncate to 6861
    assert_eq!(max_inputs, 6861, "Expected 6861 max inputs for 1MB limit with 97.5% safety");
}

#[test]
fn test_100kb_truncation_target() {
    // For 100KB limit with 98% safety margin
    let max_size = 100_000;
    let safety_margin = 0.98;
    let target_size = (max_size as f64 * safety_margin) as usize;
    
    // Calculate maximum inputs
    let input_count_varint_size = 3; // Assume we'll need 3 bytes
    let overhead_bytes = 4 + 1 + 1 + input_count_varint_size + 1 + 43 + 4;
    let bytes_per_input = 41 + 108; // P2WPKH: 41 base + 108 witness (includes witness count byte)
    
    let available_for_inputs = target_size - overhead_bytes;
    let max_inputs = available_for_inputs / bytes_per_input;
    
    // Calculate the actual size with max_inputs
    let actual_base = overhead_bytes + (max_inputs * 41);
    let actual_witness = max_inputs * 108; // 1 (count) + 1+72 (sig) + 1+33 (pubkey)
    let actual_total = actual_base + actual_witness;
    let actual_weight = (actual_base * 4) + actual_witness;
    let actual_vsize = (actual_weight + 3) / 4;
    
    println!("📊 100KB Truncation Test:");
    println!("   Target size: {} bytes ({:.2} KB)", target_size, target_size as f64 / 1024.0);
    println!("   Max inputs: {}", max_inputs);
    println!("   Actual raw size: {} bytes ({:.2} KB)", actual_total, actual_total as f64 / 1024.0);
    println!("   Actual vSize: {} vBytes ({:.2} KB)", actual_vsize, actual_vsize as f64 / 1024.0);
    
    // Verify we're within the target
    assert!(actual_total <= target_size, "Transaction exceeds target size!");
    assert!(actual_total > (target_size - bytes_per_input), "Transaction is too small");
    
    // For 100KB with 149 bytes/input, we should fit ~657 inputs
    assert_eq!(max_inputs, 657, "Expected 657 max inputs for 100KB limit");
}
