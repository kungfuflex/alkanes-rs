//! Subfrost Address Derivation and Unwrap Utilities
//!
//! This module provides:
//! 1. Functionality to derive the Subfrost (frBTC) signer address
//!    by calling the GET_SIGNER opcode (103) on the frBTC contract at [32:0].
//! 2. Minimum unwrap calculation to determine the smallest frBTC amount
//!    that will be processed by subfrost based on fee rates and premium.
//!
//! This matches the reference TypeScript implementation in:
//! ./reference/derive-subfrost-address-master/src.ts/index.ts
//!
//! The minimum unwrap calculation matches the logic in:
//! ./reference/subfrost/crates/subfrost-cli/src/unwrap.rs

use crate::proto::alkanes::MessageContextParcel;
use crate::{Result, AlkanesError};
use bitcoin::{Address, Network};
use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::taproot::TaprootSpendInfo;
use prost::Message;
use serde::{Deserialize, Serialize};

/// The GET_SIGNER opcode for frBTC contract
pub const GET_SIGNER_OPCODE: u64 = 103;

/// The frBTC contract location [32, 0]
pub const FRBTC_CONTRACT_BLOCK: u64 = 32;
pub const FRBTC_CONTRACT_TX: u64 = 0;

/// Build a MessageContextParcel for calling GET_SIGNER on frBTC [32:0]
///
/// This creates the exact protobuf structure that matches the TypeScript reference:
/// ```typescript
/// {
///   alkanes: [],
///   height: 880000,
///   vout: 0,
///   target: { block: 32n, tx: 0n },
///   inputs: [103n],
///   pointer: 0,
///   refundPointer: 0,
///   block: Buffer.from([]),
///   transaction: Buffer.from([])
/// }
/// ```
///
/// Expected protobuf encoding: `0x2080db352a03200067`
pub fn build_get_signer_parcel() -> MessageContextParcel {
    let mut parcel = MessageContextParcel::default();
    
    // Set context parameters
    parcel.height = 880000;
    parcel.vout = 0;
    parcel.pointer = 0;
    parcel.refund_pointer = 0;
    parcel.txindex = 0;
    
    // Encode target [32, 0] and GET_SIGNER opcode (103) as calldata
    // The calldata format is: [target_block_lo_byte, target_tx_lo_byte, input_opcode]
    parcel.calldata = vec![
        FRBTC_CONTRACT_BLOCK as u8,  // 32
        FRBTC_CONTRACT_TX as u8,      // 0
        GET_SIGNER_OPCODE as u8,      // 103
    ];
    
    // Empty alkanes list (no transfers)
    parcel.alkanes = vec![];
    
    // Empty block and transaction
    parcel.block = vec![];
    parcel.transaction = vec![];
    
    parcel
}

/// Encode the GET_SIGNER request as hex string with 0x prefix
///
/// Returns the hex-encoded protobuf bytes that should be passed to metashrew_view
pub fn encode_get_signer_request() -> String {
    let parcel = build_get_signer_parcel();
    let encoded_bytes = parcel.encode_to_vec();
    format!("0x{}", hex::encode(&encoded_bytes))
}

/// Parse the signer pubkey from a simulate response
///
/// The response is a hex-encoded protobuf ExtendedCallResponse which contains the pubkey in the data field.
/// Response format: `"0x<hex_encoded_protobuf>"`
pub fn parse_signer_pubkey(response: &serde_json::Value) -> Result<Vec<u8>> {
    // The response is either:
    // 1. A string with hex-encoded protobuf: "0x..."
    // 2. An object with execution.data: { "execution": { "data": "0x..." } }
    
    let hex_str = if let Some(s) = response.as_str() {
        // Direct hex string response
        s
    } else if let Some(exec_data) = response.get("execution").and_then(|e| e.get("data")).and_then(|d| d.as_str()) {
        // Nested object response
        exec_data
    } else {
        return Err(AlkanesError::RpcError("Failed to get signer pubkey from simulate result: unexpected response format".to_string()));
    };
    
    // Remove 0x prefix if present
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    
    // Decode hex to bytes
    let response_bytes = hex::decode(hex_str)
        .map_err(|e| AlkanesError::Other(format!("Failed to decode response hex: {}", e)))?;
    
    // The response is a protobuf with the pubkey in field 1, subfield 3 (data field of ExtendedCallResponse)
    // Manual extraction: skip the outer message wrapper and find field 3
    // Format: 0a <length> 1a 20 <32_bytes>
    // We need to extract the 32 bytes after the field 3 tag (0x1a) and length (0x20)
    
    let pubkey_bytes = if response_bytes.len() >= 36 && response_bytes[0] == 0x0a && response_bytes[2] == 0x1a && response_bytes[3] == 0x20 {
        // Standard format: field 1 wrapper, field 3 with 32 bytes
        response_bytes[4..36].to_vec()
    } else {
        // Try to decode as ExtendedCallResponse (may fail if storage field is malformed)
        use crate::proto::alkanes::ExtendedCallResponse;
        use prost::Message as _;
        
        let call_response = ExtendedCallResponse::decode(&response_bytes[..])
            .map_err(|e| AlkanesError::Protobuf(format!("Failed to decode ExtendedCallResponse: {}", e)))?;
        
        call_response.data
    };
    
    // Validate length
    if pubkey_bytes.len() != 32 {
        return Err(AlkanesError::Other(format!(
            "Invalid pubkey length: expected 32 bytes, got {}",
            pubkey_bytes.len()
        )));
    }
    
    Ok(pubkey_bytes)
}

/// Compute P2TR address from internal pubkey
///
/// Creates a taproot address from the x-only public key, matching the bitcoinjs-lib
/// behavior in the TypeScript reference:
/// ```typescript
/// bitcoin.payments.p2tr({ internalPubkey, network })
/// ```
pub fn compute_address(pubkey_bytes: &[u8], network: Network) -> Result<Address> {
    if pubkey_bytes.len() != 32 {
        return Err(AlkanesError::Other(format!(
            "Invalid pubkey length: expected 32 bytes, got {}",
            pubkey_bytes.len()
        )));
    }
    
    let xonly_pubkey = XOnlyPublicKey::from_slice(pubkey_bytes)
        .map_err(|e| AlkanesError::Other(format!("Failed to create XOnlyPublicKey: {}", e)))?;
    
    let secp = Secp256k1::new();
    let taproot_spend_info = TaprootSpendInfo::new_key_spend(&secp, xonly_pubkey, None);
    let address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);
    
    Ok(address)
}

/// Get the subfrost signer address for a given alkane (typically frBTC at {32, 0})
///
/// This is a convenience function that:
/// 1. Builds the GET_SIGNER request
/// 2. Calls metashrew_view with "simulate"
/// 3. Parses the pubkey from the response
/// 4. Computes the P2TR address
///
/// # Arguments
/// * `provider` - The DeezelProvider to use for the simulate call
/// * `alkane_id` - The alkane ID to query (usually {32, 0} for frBTC)
///
/// # Returns
/// The subfrost signer address as a string
pub async fn get_subfrost_address<P: crate::DeezelProvider + ?Sized>(
    provider: &P,
    alkane_id: &crate::alkanes::types::AlkaneId,
) -> Result<String> {
    // Build the request parcel with the target alkane encoded in calldata
    let parcel = build_get_signer_parcel();
    
    // Call simulate - the alkane ID doesn't matter for GET_SIGNER since it's encoded in calldata
    let response = provider.simulate("", &parcel, None).await?;
    
    // Parse the signer pubkey from JSON response
    let pubkey_bytes = parse_signer_pubkey(&response)?;
    
    // Compute the address
    let network = provider.get_network();
    let address = compute_address(&pubkey_bytes, network)?;
    
    Ok(address.to_string())
}

// ============================================================================
// Minimum Unwrap Calculation
// ============================================================================

/// Bitcoin dust threshold in satoshis
pub const DUST_THRESHOLD: u64 = 546;

/// Default unwrap premium (0.1%)
pub const DEFAULT_UNWRAP_PREMIUM: f64 = 0.001;

/// Bytes per input in a P2TR transaction (approximate)
pub const BYTES_PER_INPUT: usize = 68;

/// Bytes per output in a P2TR transaction (approximate)
pub const BYTES_PER_OUTPUT: usize = 43;

/// Base transaction overhead in bytes
pub const TX_OVERHEAD_BYTES: usize = 10;

/// Result of the minimum unwrap calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimumUnwrapResult {
    /// The minimum unwrap amount in satoshis
    pub minimum_unwrap_sats: u64,
    /// The minimum unwrap amount in BTC
    pub minimum_unwrap_btc: f64,
    /// The fee rate used for calculation (sat/vB)
    pub fee_rate: f64,
    /// The premium percentage used
    pub premium_percent: f64,
    /// Estimated transaction fee in satoshis
    pub estimated_fee_sats: u64,
    /// Fee per output in satoshis
    pub fee_per_output_sats: u64,
    /// Expected number of inputs
    pub expected_inputs: usize,
    /// Expected number of outputs
    pub expected_outputs: usize,
    /// Breakdown of the calculation
    pub breakdown: MinimumUnwrapBreakdown,
}

/// Breakdown of how the minimum unwrap was calculated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimumUnwrapBreakdown {
    /// Dust threshold that must be met
    pub dust_threshold: u64,
    /// Formula used for calculation
    pub formula: String,
    /// Explanation of the calculation
    pub explanation: String,
}

/// Calculate the estimated transaction fee for an aggregate unwrap transaction.
///
/// Uses the same formula as subfrost:
/// `fee = ((inputs + 1) * 68 + outputs * 43 + 10) * fee_rate`
///
/// The +1 on inputs accounts for potential additional UTXOs needed for fees.
pub fn estimate_transaction_fee(
    num_inputs: usize,
    num_outputs: usize,
    fee_rate: f64,
) -> u64 {
    let estimated_vsize = (num_inputs + 1) * BYTES_PER_INPUT
        + num_outputs * BYTES_PER_OUTPUT
        + TX_OVERHEAD_BYTES;
    (estimated_vsize as f64 * fee_rate) as u64
}

/// Calculate the fee per output for an aggregate unwrap transaction.
///
/// The total fee is divided equally among all outputs.
pub fn calculate_fee_per_output(
    num_inputs: usize,
    num_outputs: usize,
    fee_rate: f64,
) -> u64 {
    if num_outputs == 0 {
        return 0;
    }
    let total_fee = estimate_transaction_fee(num_inputs, num_outputs, fee_rate);
    total_fee / num_outputs as u64
}

/// Calculate the minimum unwrap amount that will be processed by subfrost.
///
/// An unwrap is skipped if either:
/// 1. The amount is too small to cover `premium + fee_per_output`
/// 2. The resulting output value is below the dust threshold (546 sats)
///
/// The calculation solves for the minimum `value` where:
/// ```text
/// final_value = value - (value * premium) - fee_per_output >= dust_threshold
/// value * (1 - premium) >= dust_threshold + fee_per_output
/// value >= (dust_threshold + fee_per_output) / (1 - premium)
/// ```
///
/// # Arguments
/// * `fee_rate` - Fee rate in sat/vB
/// * `premium` - Premium percentage as decimal (e.g., 0.001 for 0.1%)
/// * `expected_inputs` - Expected number of inputs in the aggregate transaction
/// * `expected_outputs` - Expected number of outputs in the aggregate transaction
///
/// # Returns
/// The minimum unwrap amount in satoshis
pub fn calculate_minimum_unwrap(
    fee_rate: f64,
    premium: f64,
    expected_inputs: usize,
    expected_outputs: usize,
) -> MinimumUnwrapResult {
    let fee_per_output = calculate_fee_per_output(expected_inputs, expected_outputs, fee_rate);
    let estimated_fee = estimate_transaction_fee(expected_inputs, expected_outputs, fee_rate);

    // Solve: value * (1 - premium) >= dust_threshold + fee_per_output
    // value >= (dust_threshold + fee_per_output) / (1 - premium)
    let divisor = 1.0 - premium;
    let minimum_value = if divisor > 0.0 {
        ((DUST_THRESHOLD + fee_per_output) as f64 / divisor).ceil() as u64
    } else {
        // Edge case: 100% premium (unrealistic but handle it)
        u64::MAX
    };

    // Ensure we're above the amount needed to cover premium + fee
    // value > premium * value + fee_per_output
    // value * (1 - premium) > fee_per_output
    // value > fee_per_output / (1 - premium)
    let minimum_to_cover_costs = if divisor > 0.0 {
        (fee_per_output as f64 / divisor).ceil() as u64 + 1
    } else {
        u64::MAX
    };

    // Take the maximum of both constraints
    let minimum_unwrap_sats = minimum_value.max(minimum_to_cover_costs);

    let formula = format!(
        "minimum = ceil((dust_threshold + fee_per_output) / (1 - premium))\n\
         minimum = ceil(({} + {}) / (1 - {}))\n\
         minimum = ceil({} / {})\n\
         minimum = {}",
        DUST_THRESHOLD,
        fee_per_output,
        premium,
        DUST_THRESHOLD + fee_per_output,
        divisor,
        minimum_unwrap_sats
    );

    let explanation = format!(
        "For an unwrap to be processed by subfrost:\n\
         1. The unwrap amount must cover: premium ({:.2}%) + share of miner fee ({} sats)\n\
         2. The remaining output must be >= dust threshold ({} sats)\n\n\
         At {:.2} sat/vB with {} expected inputs and {} expected outputs:\n\
         - Estimated total tx fee: {} sats\n\
         - Fee per output: {} sats\n\
         - Minimum unwrap: {} sats ({:.8} BTC)",
        premium * 100.0,
        fee_per_output,
        DUST_THRESHOLD,
        fee_rate,
        expected_inputs,
        expected_outputs,
        estimated_fee,
        fee_per_output,
        minimum_unwrap_sats,
        minimum_unwrap_sats as f64 / 100_000_000.0
    );

    MinimumUnwrapResult {
        minimum_unwrap_sats,
        minimum_unwrap_btc: minimum_unwrap_sats as f64 / 100_000_000.0,
        fee_rate,
        premium_percent: premium * 100.0,
        estimated_fee_sats: estimated_fee,
        fee_per_output_sats: fee_per_output,
        expected_inputs,
        expected_outputs,
        breakdown: MinimumUnwrapBreakdown {
            dust_threshold: DUST_THRESHOLD,
            formula,
            explanation,
        },
    }
}

/// Check if a given unwrap amount will be processed by subfrost.
///
/// Returns `Ok(final_value)` if the unwrap will be processed, where `final_value`
/// is the amount that will be sent to the recipient after premium and fees.
///
/// Returns `Err` with an explanation if the unwrap will be skipped.
pub fn check_unwrap_processable(
    amount_sats: u64,
    fee_rate: f64,
    premium: f64,
    expected_inputs: usize,
    expected_outputs: usize,
) -> std::result::Result<u64, String> {
    let fee_per_output = calculate_fee_per_output(expected_inputs, expected_outputs, fee_rate);
    let premium_amount = (amount_sats as f64 * premium) as u64;

    // Check if amount covers premium + fee
    if amount_sats <= premium_amount + fee_per_output {
        return Err(format!(
            "Unwrap of {} sats is too small to cover premium ({} sats) + fee ({} sats) = {} sats",
            amount_sats,
            premium_amount,
            fee_per_output,
            premium_amount + fee_per_output
        ));
    }

    let final_value = amount_sats - premium_amount - fee_per_output;

    // Check if final value is above dust threshold
    if final_value < DUST_THRESHOLD {
        return Err(format!(
            "Unwrap of {} sats results in {} sats after premium and fees, which is below dust threshold ({})",
            amount_sats,
            final_value,
            DUST_THRESHOLD
        ));
    }

    Ok(final_value)
}

// ============================================================================
// Command Handler
// ============================================================================

/// Execute the minimum-unwrap command.
///
/// If `fee_rate_override` is provided, it will be used directly.
/// Otherwise, this will fetch the current 6-block fee estimate from the network.
///
/// # Arguments
/// * `provider` - A provider implementing EsploraProvider for fee estimates
/// * `fee_rate_override` - Optional fee rate in sat/vB to use instead of network fetch
/// * `premium` - Premium percentage as decimal (e.g., 0.001 for 0.1%)
/// * `expected_inputs` - Expected number of inputs in the aggregate transaction
/// * `expected_outputs` - Expected number of outputs in the aggregate transaction
/// * `raw` - If true, return raw JSON; otherwise return human-readable output
///
/// # Returns
/// A formatted string with the minimum unwrap calculation result
pub async fn execute_minimum_unwrap<P: crate::traits::EsploraProvider + ?Sized>(
    provider: &P,
    fee_rate_override: Option<f64>,
    premium: f64,
    expected_inputs: usize,
    expected_outputs: usize,
    raw: bool,
) -> Result<String> {
    // Get fee rate: use override or fetch from network
    let fee_rate = if let Some(rate) = fee_rate_override {
        rate
    } else {
        // Fetch fee estimates from network
        let fee_estimates = provider.get_fee_estimates().await?;

        // Use 6-block estimate, matching subfrost behavior
        // Fall back to 1 sat/vB if not available
        fee_estimates
            .get("6")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
    };

    // Calculate minimum unwrap
    let result = calculate_minimum_unwrap(fee_rate, premium, expected_inputs, expected_outputs);

    if raw {
        // Return JSON output
        Ok(serde_json::to_string_pretty(&result)?)
    } else {
        // Return human-readable output
        Ok(format_minimum_unwrap_result(&result))
    }
}

/// Execute the minimum-unwrap command with a provided fee rate (no network fetch).
///
/// This is useful when you already have a fee rate or want to test with specific values.
pub fn execute_minimum_unwrap_with_fee_rate(
    fee_rate: f64,
    premium: f64,
    expected_inputs: usize,
    expected_outputs: usize,
    raw: bool,
) -> Result<String> {
    let result = calculate_minimum_unwrap(fee_rate, premium, expected_inputs, expected_outputs);

    if raw {
        Ok(serde_json::to_string_pretty(&result)?)
    } else {
        Ok(format_minimum_unwrap_result(&result))
    }
}

/// Execute the subfrost-thieve command to request test BTC from regtest faucet
///
/// This calls the subfrost_thieve JSON-RPC method available on subfrost regtest instances.
/// The address parameter can be either a raw Bitcoin address or a wallet address spec
/// (like "p2tr:0") which will be resolved to an actual address.
pub async fn execute_thieve<P>(
    provider: &P,
    address_spec: &str,
    amount: u64,
    raw: bool,
) -> Result<String>
where
    P: crate::traits::DeezelProvider + ?Sized,
{
    use crate::address_resolver::AddressResolver;
    use crate::traits::AddressResolver as AddressResolverTrait;

    // Resolve address spec to actual Bitcoin address
    let address = if address_spec.contains(':') {
        // It's a wallet address spec like "p2tr:0", resolve it
        let resolved = provider.resolve_all_identifiers(address_spec).await?;
        resolved
    } else {
        // It's already a Bitcoin address
        address_spec.to_string()
    };

    // Call subfrost_thieve JSON-RPC method
    let result = provider.subfrost_thieve(&address, amount).await?;

    if raw {
        // Return raw JSON output
        Ok(serde_json::to_string_pretty(&result)?)
    } else {
        // Return human-readable output
        let txid = result.as_str()
            .or_else(|| result.get("txid").and_then(|v| v.as_str()))
            .unwrap_or("unknown");

        Ok(format!(
            r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                        SUBFROST REGTEST FAUCET (THIEVE)                      ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  Successfully requested {} sats                                   ║
║  To address: {}                                  ║
║                                                                              ║
║  Transaction ID:                                                             ║
║  {}                              ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#,
            amount,
            address,
            txid
        ))
    }
}

/// Format the minimum unwrap result as a human-readable string
fn format_minimum_unwrap_result(result: &MinimumUnwrapResult) -> String {
    format!(
        r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                        SUBFROST MINIMUM UNWRAP CALCULATOR                    ║
╠══════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  Minimum Unwrap Amount:  {:>12} sats  ({:.8} BTC)            ║
║                                                                              ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  PARAMETERS                                                                  ║
╠──────────────────────────────────────────────────────────────────────────────╣
║  Fee Rate:           {:>8.2} sat/vB                                         ║
║  Premium:            {:>8.2}%                                                ║
║  Expected Inputs:    {:>8}                                                   ║
║  Expected Outputs:   {:>8}                                                   ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  FEE BREAKDOWN                                                               ║
╠──────────────────────────────────────────────────────────────────────────────╣
║  Estimated Total Fee:    {:>8} sats                                         ║
║  Fee Per Output:         {:>8} sats                                         ║
║  Dust Threshold:         {:>8} sats                                         ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  EXPLANATION                                                                 ║
╠──────────────────────────────────────────────────────────────────────────────╣
{}
╚══════════════════════════════════════════════════════════════════════════════╝
"#,
        result.minimum_unwrap_sats,
        result.minimum_unwrap_btc,
        result.fee_rate,
        result.premium_percent,
        result.expected_inputs,
        result.expected_outputs,
        result.estimated_fee_sats,
        result.fee_per_output_sats,
        result.breakdown.dust_threshold,
        result.breakdown.explanation.lines()
            .map(|line| format!("║  {}{}║", line, " ".repeat(74usize.saturating_sub(line.len()))))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_get_signer_request() {
        // This should match the exact encoding from the TypeScript reference
        let hex_encoded = encode_get_signer_request();
        
        // Expected encoding: 0x2080db352a03200067
        // Let's verify the structure is correct by decoding it
        assert!(hex_encoded.starts_with("0x"));
        
        let hex_without_prefix = hex_encoded.strip_prefix("0x").unwrap();
        let bytes = hex::decode(hex_without_prefix).unwrap();
        
        // Decode back to verify structure
        let decoded = MessageContextParcel::decode(&bytes[..]).unwrap();
        
        assert_eq!(decoded.height, 880000);
        assert_eq!(decoded.vout, 0);
        assert_eq!(decoded.pointer, 0);
        assert_eq!(decoded.refund_pointer, 0);
        assert_eq!(decoded.calldata, vec![32u8, 0u8, 103u8]);  // [target_block, target_tx, input]
        assert_eq!(decoded.alkanes.len(), 0);
        assert_eq!(decoded.block.len(), 0);
        assert_eq!(decoded.transaction.len(), 0);
        
        println!("Generated encoding: {}", hex_encoded);
        println!("Expected encoding:  0x2080db352a03200067");
        
        // Verify exact encoding match
        assert_eq!(hex_encoded, "0x2080db352a03200067");
    }
    
    #[test]
    fn test_parse_signer_pubkey() {
        // Mock response from simulate
        let response = serde_json::json!({
            "execution": {
                "data": "0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
            }
        });
        
        let pubkey_bytes = parse_signer_pubkey(&response).unwrap();
        assert_eq!(pubkey_bytes.len(), 32);
        assert_eq!(
            hex::encode(&pubkey_bytes),
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
    }
    
    #[test]
    fn test_compute_address() {
        // Test with the secp256k1 generator point (standard test key)
        let pubkey_hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pubkey_bytes = hex::decode(pubkey_hex).unwrap();

        let address = compute_address(&pubkey_bytes, Network::Regtest).unwrap();

        // This should produce a valid regtest P2TR address
        assert!(address.to_string().starts_with("bcrt1p"));

        println!("Computed address: {}", address);
    }

    #[test]
    fn test_estimate_transaction_fee() {
        // Test with typical values
        // 10 inputs, 10 outputs, 10 sat/vB
        // vsize = (10+1)*68 + 10*43 + 10 = 748 + 430 + 10 = 1188
        // fee = 1188 * 10 = 11880
        let fee = estimate_transaction_fee(10, 10, 10.0);
        assert_eq!(fee, 11880);

        // Test with 1 input, 1 output, 1 sat/vB
        // vsize = (1+1)*68 + 1*43 + 10 = 136 + 43 + 10 = 189
        // fee = 189 * 1 = 189
        let fee = estimate_transaction_fee(1, 1, 1.0);
        assert_eq!(fee, 189);
    }

    #[test]
    fn test_calculate_fee_per_output() {
        // 10 inputs, 10 outputs, 10 sat/vB
        // total fee = 11880 sats
        // fee per output = 11880 / 10 = 1188
        let fee_per_output = calculate_fee_per_output(10, 10, 10.0);
        assert_eq!(fee_per_output, 1188);
    }

    #[test]
    fn test_calculate_minimum_unwrap() {
        // Test with typical values: 10 sat/vB, 0.1% premium, 10 inputs, 10 outputs
        let result = calculate_minimum_unwrap(10.0, 0.001, 10, 10);

        // fee_per_output = 1188 sats
        // minimum = ceil((546 + 1188) / (1 - 0.001)) = ceil(1734 / 0.999) = ceil(1735.73) = 1736
        assert!(result.minimum_unwrap_sats >= 1736);

        // Verify the minimum actually works
        let check = check_unwrap_processable(
            result.minimum_unwrap_sats,
            10.0,
            0.001,
            10,
            10
        );
        assert!(check.is_ok(), "Minimum should be processable");

        // Verify one less doesn't work
        let check_below = check_unwrap_processable(
            result.minimum_unwrap_sats - 1,
            10.0,
            0.001,
            10,
            10
        );
        assert!(check_below.is_err(), "Below minimum should not be processable");

        println!("Minimum unwrap at 10 sat/vB: {} sats", result.minimum_unwrap_sats);
        println!("Explanation: {}", result.breakdown.explanation);
    }

    #[test]
    fn test_calculate_minimum_unwrap_high_fees() {
        // Test with high fees: 100 sat/vB
        let result = calculate_minimum_unwrap(100.0, 0.001, 10, 10);

        // Higher fees mean higher minimum
        assert!(result.minimum_unwrap_sats > 10000);

        // Verify it works
        let check = check_unwrap_processable(
            result.minimum_unwrap_sats,
            100.0,
            0.001,
            10,
            10
        );
        assert!(check.is_ok());

        println!("Minimum unwrap at 100 sat/vB: {} sats ({:.8} BTC)",
                 result.minimum_unwrap_sats, result.minimum_unwrap_btc);
    }

    #[test]
    fn test_check_unwrap_processable() {
        // Test a clearly processable amount
        let result = check_unwrap_processable(100000, 10.0, 0.001, 10, 10);
        assert!(result.is_ok());
        let final_value = result.unwrap();
        // final = 100000 - 100 (premium 0.1%) - 1188 (fee) = 98712
        assert!(final_value > 0);
        assert!(final_value >= DUST_THRESHOLD);

        // Test a clearly too small amount
        let result = check_unwrap_processable(500, 10.0, 0.001, 10, 10);
        assert!(result.is_err());
    }
}
