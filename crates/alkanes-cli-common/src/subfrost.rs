//! Subfrost Address Derivation
//!
//! This module provides functionality to derive the Subfrost (frBTC) signer address
//! by calling the GET_SIGNER opcode (103) on the frBTC contract at [32:0].
//!
//! This matches the reference TypeScript implementation in:
//! ./reference/derive-subfrost-address-master/src.ts/index.ts

use crate::proto::alkanes::MessageContextParcel;
use crate::{Result, AlkanesError};
use bitcoin::{Address, Network};
use bitcoin::key::XOnlyPublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::taproot::TaprootSpendInfo;
use prost::Message;

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
}
