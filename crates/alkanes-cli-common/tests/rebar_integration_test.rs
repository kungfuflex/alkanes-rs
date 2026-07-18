/// Tests for --use-rebar integration
/// 
/// Rebar Shield requires:
/// 1. Query /v1/info to get payment address and fee tiers
/// 2. Add extra output to transaction for Rebar payment
/// 3. Payment = transaction_vsize × fee_tier_rate
/// 4. Submit via JSON-RPC to https://shield.rebarlabs.io/v1/rpc
/// 
/// Key difference from Slipstream: Requires payment OUTPUT instead of just higher fee

#[cfg(test)]
mod rebar_integration_tests {
    use serde_json::json;

    /// Rebar Shield fee tier information
    #[derive(Debug, Clone)]
    struct RebarFeeTier {
        estimated_hashrate: f64,  // Percentage of network hashrate (0.0-1.0)
        feerate: u64,             // Fee rate in sat/vB
    }

    /// Rebar Shield payment information
    #[derive(Debug, Clone)]
    struct RebarPaymentInfo {
        payment_address: String,  // Address to send Rebar payment to
        block_height: u64,
        fee_tiers: Vec<RebarFeeTier>,
    }

    /// Mock Rebar /v1/info response for testing
    fn mock_rebar_info() -> RebarPaymentInfo {
        RebarPaymentInfo {
            payment_address: "bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek".to_string(),
            block_height: 920137,
            fee_tiers: vec![
                RebarFeeTier {
                    estimated_hashrate: 0.08,
                    feerate: 16,
                },
                RebarFeeTier {
                    estimated_hashrate: 0.16,
                    feerate: 28,
                },
            ],
        }
    }

    /// Calculate Rebar payment amount for a transaction
    fn calculate_rebar_payment(tx_vsize: usize, fee_tier: &RebarFeeTier) -> u64 {
        tx_vsize as u64 * fee_tier.feerate
    }

    /// Build transaction with Rebar payment output
    /// Returns (main_output_amount, rebar_payment_amount, total_fee)
    fn build_transaction_with_rebar(
        total_input: u64,
        num_inputs: usize,
        destination_addr: &str,
        rebar_info: &RebarPaymentInfo,
        tier_index: usize,
        base_fee_rate: f64,
    ) -> (u64, u64, u64) {
        // Calculate transaction size
        // With 2 outputs: main output + Rebar payment output
        let base_overhead = 10;
        let bytes_per_signed_input = 107;
        let bytes_per_output = 43; // P2TR or P2WPKH
        
        let tx_vsize = base_overhead + (num_inputs * bytes_per_signed_input) + (2 * bytes_per_output);
        
        // Calculate base Bitcoin network fee
        let base_fee = (tx_vsize as f64 * base_fee_rate).ceil() as u64;
        
        // Calculate Rebar payment (this is the "premium" paid to Rebar)
        let tier = &rebar_info.fee_tiers[tier_index];
        let rebar_payment = calculate_rebar_payment(tx_vsize, tier);
        
        // Total fee = base network fee + Rebar payment
        let total_fee = base_fee + rebar_payment;
        
        // Main output = input - total_fee
        let main_output = total_input.saturating_sub(total_fee);
        
        (main_output, rebar_payment, total_fee)
    }

    #[test]
    fn test_rebar_payment_calculation() {
        // Test Rebar payment calculation for different transaction sizes
        let rebar_info = mock_rebar_info();
        
        // Small transaction: 100 inputs
        let small_vsize = 53 + (100 * 107);
        let payment_small_t1 = calculate_rebar_payment(small_vsize, &rebar_info.fee_tiers[0]);
        let payment_small_t2 = calculate_rebar_payment(small_vsize, &rebar_info.fee_tiers[1]);
        
        println!("Small tx (100 inputs, {} vbytes):", small_vsize);
        println!("  Tier 1 (16 sat/vB): {} sats", payment_small_t1);
        println!("  Tier 2 (28 sat/vB): {} sats", payment_small_t2);
        
        assert_eq!(payment_small_t1, small_vsize as u64 * 16);
        assert_eq!(payment_small_t2, small_vsize as u64 * 28);
        
        // Large transaction: 9,158 inputs (max for 1MB)
        let large_vsize = 53 + (9158 * 107);
        let payment_large_t1 = calculate_rebar_payment(large_vsize, &rebar_info.fee_tiers[0]);
        let payment_large_t2 = calculate_rebar_payment(large_vsize, &rebar_info.fee_tiers[1]);
        
        println!("\nLarge tx (9,158 inputs, {} vbytes):", large_vsize);
        println!("  Tier 1 (16 sat/vB): {} sats ({:.4} BTC)", payment_large_t1, payment_large_t1 as f64 / 100_000_000.0);
        println!("  Tier 2 (28 sat/vB): {} sats ({:.4} BTC)", payment_large_t2, payment_large_t2 as f64 / 100_000_000.0);
        
        // For consolidation, Rebar payment is VERY expensive for large txs
        assert!(payment_large_t1 > 15_000_000, "Rebar payment for large tx is significant");
    }

    #[test]
    fn test_consolidation_with_rebar() {
        let rebar_info = mock_rebar_info();
        
        // Scenario: Consolidate 9,158 UTXOs @ 1,999 sats each
        let num_inputs = 9158;
        let sats_per_input = 1999;
        let total_input = num_inputs as u64 * sats_per_input;
        let base_fee_rate = 2.1; // Base network fee
        
        println!("=== Consolidation with Rebar Shield ===");
        println!("Total input: {} sats ({:.8} BTC)", total_input, total_input as f64 / 100_000_000.0);
        println!("Inputs: {}", num_inputs);
        println!("");
        
        // Calculate for Tier 1 (8% hashrate)
        let (main_output_t1, rebar_payment_t1, total_fee_t1) = build_transaction_with_rebar(
            total_input,
            num_inputs,
            "bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05",
            &rebar_info,
            0, // Tier 1
            base_fee_rate,
        );
        
        println!("Tier 1 (16 sat/vB @ 8% hashrate):");
        println!("  Main output: {} sats ({:.8} BTC)", main_output_t1, main_output_t1 as f64 / 100_000_000.0);
        println!("  Rebar payment: {} sats ({:.8} BTC)", rebar_payment_t1, rebar_payment_t1 as f64 / 100_000_000.0);
        println!("  Total fee: {} sats ({:.8} BTC)", total_fee_t1, total_fee_t1 as f64 / 100_000_000.0);
        println!("  Effective rate: {:.2} sat/vB", total_fee_t1 as f64 / (53 + num_inputs * 107) as f64);
        println!("");
        
        // Calculate for Tier 2 (16% hashrate)
        let (main_output_t2, rebar_payment_t2, total_fee_t2) = build_transaction_with_rebar(
            total_input,
            num_inputs,
            "bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05",
            &rebar_info,
            1, // Tier 2
            base_fee_rate,
        );
        
        println!("Tier 2 (28 sat/vB @ 16% hashrate):");
        println!("  Main output: {} sats ({:.8} BTC)", main_output_t2, main_output_t2 as f64 / 100_000_000.0);
        println!("  Rebar payment: {} sats ({:.8} BTC)", rebar_payment_t2, rebar_payment_t2 as f64 / 100_000_000.0);
        println!("  Total fee: {} sats ({:.8} BTC)", total_fee_t2, total_fee_t2 as f64 / 100_000_000.0);
        println!("  Effective rate: {:.2} sat/vB", total_fee_t2 as f64 / (53 + num_inputs * 107) as f64);
        println!("");
        
        // Verify we're not sending more than we have
        assert!(main_output_t1 > 0, "Should have funds left after Rebar payment");
        assert!(main_output_t1 + rebar_payment_t1 <= total_input, "Outputs should not exceed input");
        
        // Note: Tier 2 is VERY expensive for large transactions
        println!("⚠️  WARNING: Rebar is expensive for large transactions!");
        println!("   Tier 1 payment alone: {:.2}% of total input", rebar_payment_t1 as f64 / total_input as f64 * 100.0);
        println!("   Tier 2 payment alone: {:.2}% of total input", rebar_payment_t2 as f64 / total_input as f64 * 100.0);
    }

    #[test]
    fn test_rebar_json_rpc_format() {
        // Test the JSON-RPC request format for Rebar Shield
        let tx_hex = "0200000001..."; // Sample hex
        
        let request = json!({
            "jsonrpc": "2.0",
            "id": "alkanes-cli",
            "method": "sendrawtransaction",
            "params": [tx_hex]
        });
        
        println!("Rebar Shield JSON-RPC request:");
        println!("{}", serde_json::to_string_pretty(&request).unwrap());
        
        // Expected response on success
        let success_response = json!({
            "result": "txid_hex_string",
            "error": null,
            "id": "alkanes-cli"
        });
        
        println!("\nExpected success response:");
        println!("{}", serde_json::to_string_pretty(&success_response).unwrap());
        
        // Expected response on error
        let error_response = json!({
            "result": null,
            "error": {
                "code": -26,
                "message": "tx-size"
            },
            "id": "alkanes-cli"
        });
        
        println!("\nExpected error response:");
        println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
    }

    #[test]
    fn test_compare_slipstream_vs_rebar() {
        // Compare costs between Slipstream and Rebar for our consolidation
        let num_inputs = 9158;
        let sats_per_input = 1999;
        let total_input = num_inputs as u64 * sats_per_input;
        let tx_vsize = 53 + (num_inputs * 107);
        
        println!("=== Comparison: Slipstream vs Rebar Shield ===");
        println!("Transaction: {} inputs, {} vbytes", num_inputs, tx_vsize);
        println!("Total input: {} sats ({:.8} BTC)", total_input, total_input as f64 / 100_000_000.0);
        println!("");
        
        // Slipstream: Just higher fee rate, no payment output
        let slipstream_fee_rate = 2.1; // sat/vB (minimum is 2.0)
        let slipstream_fee = (tx_vsize as f64 * slipstream_fee_rate).ceil() as u64;
        let slipstream_output = total_input - slipstream_fee;
        
        println!("SLIPSTREAM (2.1 sat/vB):");
        println!("  Fee: {} sats", slipstream_fee);
        println!("  Output: {} sats ({:.8} BTC)", slipstream_output, slipstream_output as f64 / 100_000_000.0);
        println!("  Cost: {:.2}% of input", slipstream_fee as f64 / total_input as f64 * 100.0);
        println!("");
        
        // Rebar Tier 1: Base fee + payment output
        let rebar_base_fee = (tx_vsize as f64 * 2.1).ceil() as u64;
        let rebar_payment_t1 = tx_vsize as u64 * 16; // Tier 1 rate
        let rebar_total_t1 = rebar_base_fee + rebar_payment_t1;
        let rebar_output_t1 = total_input.saturating_sub(rebar_total_t1);
        
        println!("REBAR TIER 1 (16 sat/vB @ 8% hashrate):");
        println!("  Base fee: {} sats", rebar_base_fee);
        println!("  Rebar payment: {} sats", rebar_payment_t1);
        println!("  Total cost: {} sats", rebar_total_t1);
        println!("  Output: {} sats ({:.8} BTC)", rebar_output_t1, rebar_output_t1 as f64 / 100_000_000.0);
        println!("  Cost: {:.2}% of input", rebar_total_t1 as f64 / total_input as f64 * 100.0);
        println!("");
        
        // Rebar Tier 2: Base fee + higher payment output
        let rebar_payment_t2 = tx_vsize as u64 * 28; // Tier 2 rate
        let rebar_total_t2 = rebar_base_fee + rebar_payment_t2;
        let rebar_output_t2 = total_input.saturating_sub(rebar_total_t2);
        
        println!("REBAR TIER 2 (28 sat/vB @ 16% hashrate):");
        println!("  Base fee: {} sats", rebar_base_fee);
        println!("  Rebar payment: {} sats", rebar_payment_t2);
        println!("  Total cost: {} sats", rebar_total_t2);
        println!("  Output: {} sats ({:.8} BTC)", rebar_output_t2, rebar_output_t2 as f64 / 100_000_000.0);
        println!("  Cost: {:.2}% of input", rebar_total_t2 as f64 / total_input as f64 * 100.0);
        println!("");
        
        // Summary
        println!("=== SUMMARY ===");
        println!("Slipstream is cheaper for large transactions:");
        println!("  Slipstream: {} sats cost", slipstream_fee);
        println!("  Rebar T1:   {} sats cost ({}x more expensive)", rebar_total_t1, rebar_total_t1 / slipstream_fee);
        println!("  Rebar T2:   {} sats cost ({}x more expensive)", rebar_total_t2, rebar_total_t2 / slipstream_fee);
        println!("");
        println!("Recommendation: Use Slipstream for large consolidations");
        
        // Verify calculations are sane
        assert!(slipstream_output > 0, "Slipstream should leave positive output");
        assert!(rebar_output_t1 > 0 || rebar_payment_t1 > total_input / 2, 
                "Rebar T1: Either positive output or payment is very large");
    }

    #[test]
    fn test_rebar_transaction_structure() {
        // Test the structure of a transaction with Rebar payment
        let rebar_info = mock_rebar_info();
        let num_inputs = 100; // Smaller example for clarity
        let total_input = num_inputs * 1999;
        
        let (main_output, rebar_payment, total_fee) = build_transaction_with_rebar(
            total_input as u64,
            num_inputs,
            "bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05",
            &rebar_info,
            0, // Tier 1
            2.1,
        );
        
        println!("Transaction structure:");
        println!("  Inputs: {} (each 1999 sats)", num_inputs);
        println!("  Total input: {} sats", total_input);
        println!("");
        println!("  Output 0 (main): {} sats", main_output);
        println!("  Output 1 (Rebar): {} sats to {}", rebar_payment, rebar_info.payment_address);
        println!("");
        println!("  Total outputs: {} sats", main_output + rebar_payment);
        println!("  Total fee (network): {} sats", total_fee - rebar_payment);
        println!("  Total cost: {} sats", total_fee);
        
        // Verify outputs don't exceed input
        assert_eq!(main_output + rebar_payment + (total_fee - rebar_payment), total_input as u64);
    }

    #[test]
    fn test_rebar_vs_slipstream_breakeven() {
        // Find the transaction size where Rebar becomes more expensive than Slipstream
        let rebar_info = mock_rebar_info();
        
        println!("=== Breakeven Analysis ===");
        println!("Finding transaction size where Rebar Tier 1 costs more than Slipstream");
        println!("");
        
        for num_inputs in [10, 50, 100, 500, 1000, 5000, 9158].iter() {
            let tx_vsize = 53 + (num_inputs * 107);
            
            // Slipstream cost
            let slipstream_cost = (tx_vsize as f64 * 2.1).ceil() as u64;
            
            // Rebar Tier 1 cost (base fee + payment)
            let rebar_base = (tx_vsize as f64 * 2.1).ceil() as u64;
            let rebar_payment = tx_vsize as u64 * 16;
            let rebar_total = rebar_base + rebar_payment;
            
            let ratio = rebar_total as f64 / slipstream_cost as f64;
            
            println!("{:5} inputs: Slipstream={:8} sats, Rebar={:9} sats ({:.1}x)", 
                    num_inputs, slipstream_cost, rebar_total, ratio);
        }
        
        println!("");
        println!("Conclusion: Rebar is always ~8.6x more expensive due to 16 sat/vB payment");
    }

    #[test]
    fn test_rebar_json_rpc_request_format() {
        // Test that we can construct the correct JSON-RPC request
        let tx_hex = "020000000001...";
        
        let request = json!({
            "jsonrpc": "2.0",
            "id": "alkanes-cli-consolidation",
            "method": "sendrawtransaction",
            "params": [tx_hex]
        });
        
        // Verify structure
        assert_eq!(request["jsonrpc"], "2.0");
        assert_eq!(request["method"], "sendrawtransaction");
        assert!(request["params"].is_array());
        assert_eq!(request["params"].as_array().unwrap().len(), 1);
        
        println!("✓ JSON-RPC request format is correct");
        println!("{}", serde_json::to_string_pretty(&request).unwrap());
    }

    #[test]
    fn test_rebar_implementation_checklist() {
        println!("=== Implementation Checklist for --use-rebar ===");
        println!("");
        println!("1. Add --use-rebar flag to:");
        println!("   - WalletCommands::Send");
        println!("   - WalletCommands::SendAll");
        println!("   - BitcoindCommands::Sendrawtransaction");
        println!("");
        println!("2. When --use-rebar is set during 'wallet send':");
        println!("   a) Query https://shield.rebarlabs.io/v1/info");
        println!("   b) Extract payment address and fee tier");
        println!("   c) Calculate Rebar payment = tx_vsize × tier_feerate");
        println!("   d) Add second output to transaction:");
        println!("      - Output 0: Main destination");
        println!("      - Output 1: Rebar payment address");
        println!("   e) Adjust main output = input - base_fee - rebar_payment");
        println!("");
        println!("3. When --use-rebar is set during 'sendrawtransaction':");
        println!("   a) Use JSON-RPC POST to https://shield.rebarlabs.io/v1/rpc");
        println!("   b) Standard Bitcoin Core RPC format");
        println!("   c) Parse response for txid");
        println!("");
        println!("4. Optional flags:");
        println!("   --rebar-tier <1|2>  - Choose fee tier (default: 1)");
        println!("   --rebar-api-url <url> - Override Rebar endpoint");
        println!("");
        println!("5. Validation:");
        println!("   - Verify transaction has Rebar payment output");
        println!("   - Check payment amount matches tier rate");
        println!("   - Warn user about total cost vs Slipstream");
    }
}
