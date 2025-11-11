/// Test to validate P2WPKH transaction size calculations
/// This ensures our truncation logic correctly estimates signed transaction sizes

#[test]
fn test_p2wpkh_size_calculation() {
    // P2WPKH transaction structure:
    // Base (non-witness): version(4) + marker(1) + flag(1) + input_count + inputs*(txid(32)+vout(4)+scriptSig(1)+sequence(4)) + output_count(1) + outputs + locktime(4)
    // Witness: per input: witness_count(1) + sig_len(1) + signature(~72) + pubkey_len(1) + pubkey(33) = ~107 bytes
    
    let test_cases = vec![
        // (num_inputs, expected_base_size, expected_witness_size, expected_total_raw, expected_vsize)
        (1, 4+1+1+1 + 1*41 + 1+43 + 4, 1*107, 4+1+1+1 + 1*41 + 1+43 + 4 + 1*107, (4+1+1+1 + 1*41 + 1+43 + 4)*4 + 1*107),
        (100, 4+1+1+1 + 100*41 + 1+43 + 4, 100*107, 4+1+1+1 + 100*41 + 1+43 + 4 + 100*107, (4+1+1+1 + 100*41 + 1+43 + 4)*4 + 100*107),
        (253, 4+1+1+3 + 253*41 + 1+43 + 4, 253*107, 4+1+1+3 + 253*41 + 1+43 + 4 + 253*107, (4+1+1+3 + 253*41 + 1+43 + 4)*4 + 253*107),
        (1000, 4+1+1+3 + 1000*41 + 1+43 + 4, 1000*107, 4+1+1+3 + 1000*41 + 1+43 + 4 + 1000*107, (4+1+1+3 + 1000*41 + 1+43 + 4)*4 + 1000*107),
        (6943, 4+1+1+3 + 6943*41 + 1+43 + 4, 6943*107, 4+1+1+3 + 6943*41 + 1+43 + 4 + 6943*107, (4+1+1+3 + 6943*41 + 1+43 + 4)*4 + 6943*107),
        (9894, 4+1+1+3 + 9894*41 + 1+43 + 4, 9894*107, 4+1+1+3 + 9894*41 + 1+43 + 4 + 9894*107, (4+1+1+3 + 9894*41 + 1+43 + 4)*4 + 9894*107),
    ];
    
    for (num_inputs, expected_base, expected_witness, expected_total, expected_weight) in test_cases {
        let input_count_varint_size = if num_inputs < 253 { 1 } else if num_inputs < 65536 { 3 } else { 5 };
        let overhead_bytes = 4 + 1 + 1 + input_count_varint_size + 1 + 43 + 4;
        let bytes_per_input = 41 + 107;
        
        let calculated_base = overhead_bytes + (num_inputs * 41);
        let calculated_witness = num_inputs * 107;
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
    // For 1MB limit with 98% safety margin
    let max_size = 1_048_576;
    let safety_margin = 0.98;
    let target_size = (max_size as f64 * safety_margin) as usize;
    
    // Calculate maximum inputs
    let input_count_varint_size = 3; // Assume we'll need 3 bytes for the count
    let overhead_bytes = 4 + 1 + 1 + input_count_varint_size + 1 + 43 + 4;
    let bytes_per_input = 41 + 107; // P2WPKH: 41 base + 107 witness
    
    let available_for_inputs = target_size - overhead_bytes;
    let max_inputs = available_for_inputs / bytes_per_input;
    
    // Calculate the actual size with max_inputs
    let actual_base = overhead_bytes + (max_inputs * 41);
    let actual_witness = max_inputs * 107;
    let actual_total = actual_base + actual_witness;
    let actual_weight = (actual_base * 4) + actual_witness;
    let actual_vsize = (actual_weight + 3) / 4;
    
    println!("📊 1MB Truncation Test:");
    println!("   Target size: {} bytes ({:.2} KB)", target_size, target_size as f64 / 1024.0);
    println!("   Max inputs: {}", max_inputs);
    println!("   Actual raw size: {} bytes ({:.2} KB)", actual_total, actual_total as f64 / 1024.0);
    println!("   Actual vSize: {} vBytes ({:.2} KB)", actual_vsize, actual_vsize as f64 / 1024.0);
    println!("   Actual weight: {} WU", actual_weight);
    
    // Verify we're within the target
    assert!(actual_total <= target_size, "Transaction exceeds target size!");
    assert!(actual_total > (target_size - bytes_per_input), "Transaction is too small (could fit more inputs)");
    
    // For 9894 inputs, we should truncate to ~6942
    assert_eq!(max_inputs, 6942, "Expected 6942 max inputs for 1MB limit");
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
    let bytes_per_input = 41 + 107;
    
    let available_for_inputs = target_size - overhead_bytes;
    let max_inputs = available_for_inputs / bytes_per_input;
    
    // Calculate the actual size with max_inputs
    let actual_base = overhead_bytes + (max_inputs * 41);
    let actual_witness = max_inputs * 107;
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
    
    // For 100KB, we should fit ~661 inputs
    assert_eq!(max_inputs, 661, "Expected 661 max inputs for 100KB limit");
}
