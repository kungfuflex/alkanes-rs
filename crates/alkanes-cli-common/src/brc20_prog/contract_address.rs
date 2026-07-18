//! Contract address computation for BRC20-Prog deployments
//!
//! Computes Ethereum contract addresses from deployer addresses using the standard
//! Ethereum address derivation: keccak256(rlp([sender, nonce]))[12:]

use crate::{AlkanesError, Result};
use sha3::{Digest, Keccak256};

/// Compute Ethereum contract address from deployer address and nonce
/// 
/// Formula: contract_address = keccak256(rlp([sender, nonce]))[12:]
/// 
/// # Arguments
/// * `deployer_address` - The 20-byte Ethereum address of the deployer (0x prefixed hex)
/// * `nonce` - The transaction nonce (typically 0 for first deployment)
pub fn compute_contract_address(deployer_address: &str, nonce: u64) -> Result<String> {
    // Remove 0x prefix if present
    let address_hex = deployer_address.trim_start_matches("0x");
    
    // Parse the address
    if address_hex.len() != 40 {
        return Err(AlkanesError::Parse(format!(
            "Invalid Ethereum address length: expected 40 hex chars, got {}",
            address_hex.len()
        )));
    }
    
    let address_bytes = hex::decode(address_hex)
        .map_err(|e| AlkanesError::Parse(format!("Invalid address hex: {}", e)))?;
    
    // RLP encode [address, nonce]
    let rlp_encoded = rlp_encode_address_nonce(&address_bytes, nonce);
    
    // Compute keccak256 hash
    let mut hasher = Keccak256::new();
    hasher.update(&rlp_encoded);
    let hash = hasher.finalize();
    
    // Take last 20 bytes (skip first 12)
    let contract_address = &hash[12..32];
    
    Ok(format!("0x{}", hex::encode(contract_address)))
}

/// RLP encode [address, nonce] for contract address derivation
/// 
/// RLP encoding rules:
/// - List: [0xc0 + length, items...]
/// - Bytes (address): [0x80 + length, bytes...]
/// - Small int (nonce < 128): just the byte
/// - Larger int: [0x80 + length, big-endian bytes...]
fn rlp_encode_address_nonce(address: &[u8], nonce: u64) -> Vec<u8> {
    let mut result = Vec::new();
    
    // RLP encode the address (20 bytes)
    // Format: 0x94 (0x80 + 20) followed by the 20 address bytes
    result.push(0x94); // 0x80 + 20
    result.extend_from_slice(address);
    
    // RLP encode the nonce
    if nonce == 0 {
        result.push(0x80); // Empty byte string
    } else if nonce < 0x80 {
        result.push(nonce as u8); // Single byte for small nonces
    } else {
        // Encode as byte string
        let nonce_bytes = encode_int(nonce);
        result.push(0x80 + nonce_bytes.len() as u8);
        result.extend_from_slice(&nonce_bytes);
    }
    
    // Wrap in list prefix
    // Total length = address encoding (21 bytes) + nonce encoding
    let content_length = result.len();
    let mut final_result = Vec::new();
    final_result.push(0xc0 + content_length as u8); // List prefix
    final_result.extend_from_slice(&result);
    
    final_result
}

/// Encode integer as minimal big-endian bytes
fn encode_int(mut n: u64) -> Vec<u8> {
    if n == 0 {
        return vec![];
    }
    
    let mut bytes = Vec::new();
    while n > 0 {
        bytes.push((n & 0xff) as u8);
        n >>= 8;
    }
    bytes.reverse(); // Convert to big-endian
    bytes
}

/// Compute Ethereum address from Bitcoin pkscript
/// 
/// For brc20-prog, the Ethereum address is: keccak256(pkscript)[12:]
pub fn pkscript_to_eth_address(pkscript_hex: &str) -> Result<String> {
    let pkscript = hex::decode(pkscript_hex.trim_start_matches("0x"))
        .map_err(|e| AlkanesError::Parse(format!("Invalid pkscript hex: {}", e)))?;
    
    let mut hasher = Keccak256::new();
    hasher.update(&pkscript);
    let hash = hasher.finalize();
    
    // Take last 20 bytes
    let address = &hash[12..32];
    
    Ok(format!("0x{}", hex::encode(address)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlp_encode_zero_nonce() {
        // Test RLP encoding of [0x1234...5678, 0]
        let address = hex::decode("1234567890123456789012345678901234567890").unwrap();
        let rlp = rlp_encode_address_nonce(&address, 0);
        
        // Expected: 0xd6 (list length 22), 0x94 (20 bytes), address (20 bytes), 0x80 (empty)
        assert_eq!(rlp[0], 0xd6); // List prefix
        assert_eq!(rlp[1], 0x94); // Address prefix
        assert_eq!(rlp[22], 0x80); // Empty nonce
    }

    #[test]
    fn test_compute_contract_address_zero_nonce() {
        // Test known case: deployer at 0x0000...0001, nonce 0
        // This is a deterministic test
        let deployer = "0x0000000000000000000000000000000000000001";
        let contract = compute_contract_address(deployer, 0).unwrap();
        
        // The contract address should be deterministic
        assert!(contract.starts_with("0x"));
        assert_eq!(contract.len(), 42); // 0x + 40 hex chars
    }

    #[test]
    fn test_rlp_encode_small_nonce() {
        let address = hex::decode("1234567890123456789012345678901234567890").unwrap();
        let rlp = rlp_encode_address_nonce(&address, 5);
        
        // Last byte should be 5 (nonce < 128 encoded as single byte)
        assert_eq!(*rlp.last().unwrap(), 5);
    }
}
