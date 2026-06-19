// BRC20-Prog envelope implementation using ord inscription format
// Unlike alkanes (BIN protocol), this uses the standard ord envelope with text/plain content-type

use crate::AlkanesError;
use alloc::format;
use anyhow::Result;
use bitcoin::{
    blockdata::opcodes,
    script::Builder as ScriptBuilder,
    taproot::ControlBlock,
    ScriptBuf, Witness, XOnlyPublicKey,
};

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

// Ord protocol constants (different from alkanes BIN protocol)
pub const ORD_PROTOCOL_ID: [u8; 3] = *b"ord";
pub const BODY_TAG: [u8; 0] = [];
pub const CONTENT_TYPE_TAG: u8 = 0x01; // Content-Type tag for ord inscriptions
const MAX_SCRIPT_ELEMENT_SIZE: usize = 520;

/// BRC20-Prog envelope structure using ord inscription format
/// This creates inscriptions compatible with the standard ord indexer
#[derive(Debug, Clone)]
pub struct Brc20ProgEnvelope {
    /// The JSON payload (e.g., {"p":"brc20-prog","op":"deploy","d":"0x..."})
    pub payload: Vec<u8>,
}

impl Brc20ProgEnvelope {
    /// Create new BRC20-prog envelope with JSON payload
    pub fn new(payload: Vec<u8>) -> Self {
        Self { payload }
    }

    /// Create envelope for BRC20-prog contract deployment
    pub fn for_deploy(json_payload: String) -> Self {
        Self::new(json_payload.into_bytes())
    }

    /// Create envelope for BRC20-prog contract call
    pub fn for_call(json_payload: String) -> Self {
        Self::new(json_payload.into_bytes())
    }

    /// Build the reveal script following ord inscription format
    /// Structure: <pubkey> CHECKSIG OP_FALSE OP_IF "ord" <content_type_tag> "text/plain" <body_tag> <payload> OP_ENDIF
    ///
    /// IMPORTANT: The <pubkey> CHECKSIG at the start ensures ONLY the holder of the corresponding
    /// private key can spend this script-path output. Without this, anyone could spend the commit
    /// output (frontrunning vulnerability).
    pub fn build_reveal_script(&self, pubkey: XOnlyPublicKey) -> ScriptBuf {
        let mut builder = ScriptBuilder::new()
            // CRITICAL: Pubkey + CHECKSIG at the start to prevent frontrunning
            // Only the holder of the ephemeral private key can create a valid signature
            .push_x_only_key(&pubkey)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            // The inscription envelope follows - this is inside OP_FALSE OP_IF so it's not executed
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(ORD_PROTOCOL_ID); // "ord" protocol identifier

        // Add content-type tag and value
        builder = builder
            .push_slice([CONTENT_TYPE_TAG])
            .push_slice(b"text/plain;charset=utf-8");

        // Add body tag (empty slice to separate metadata from body)
        builder = builder.push_slice(BODY_TAG);

        // Add the JSON payload in chunks
        for chunk in self.payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
            builder = builder.push_slice::<&bitcoin::script::PushBytes>(chunk.try_into().unwrap());
        }

        // End with OP_ENDIF
        builder.push_opcode(opcodes::all::OP_ENDIF).into_script()
    }

    /// Create complete witness for taproot script-path spending with signature
    /// This creates the complete 3-element witness: [signature, script, control_block]
    pub fn create_complete_witness(
        &self,
        signature: &[u8],
        control_block: ControlBlock,
        pubkey: XOnlyPublicKey,
    ) -> Result<Witness, AlkanesError> {
        let reveal_script = self.build_reveal_script(pubkey);

        let mut witness = Witness::new();

        // P2TR script-path witness structure: [signature, script, control_block]

        // 1. Push the signature
        witness.push(signature);

        // 2. Push the script bytes (contains the ord envelope with JSON)
        let script_bytes = reveal_script.as_bytes();
        witness.push(script_bytes);

        // 3. Push the control block bytes
        let control_block_bytes = control_block.serialize();
        witness.push(&control_block_bytes);

        // Verify the witness was created correctly - expecting 3 items for P2TR script-path
        if witness.len() != 3 {
            return Err(AlkanesError::Other(format!(
                "Invalid witness length: expected 3 items (signature + script + control_block), got {}",
                witness.len()
            )));
        }

        // Verify all elements are non-empty
        for (i, item) in witness.iter().enumerate() {
            if item.is_empty() {
                return Err(AlkanesError::Other(format!(
                    "Witness item {i} is empty"
                )));
            }
        }

        Ok(witness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};

    fn test_pubkey() -> XOnlyPublicKey {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &secret_key);
        XOnlyPublicKey::from_keypair(&keypair).0
    }

    #[test]
    fn test_deploy_envelope_script() {
        let json = r#"{"p":"brc20-prog","op":"deploy","d":"0x608060405234801561001057600080fd5b50"}"#;
        let envelope = Brc20ProgEnvelope::for_deploy(json.to_string());
        let pubkey = test_pubkey();
        let script = envelope.build_reveal_script(pubkey);

        // Verify script starts with <pubkey> CHECKSIG OP_FALSE OP_IF "ord"
        // Script structure: [32-byte pubkey push] [CHECKSIG] [OP_FALSE] [OP_IF] [3-byte "ord" push] ...
        let script_bytes = script.as_bytes();
        assert!(script_bytes.len() > 40);
        // First byte should be 0x20 (32) for the pubkey push
        assert_eq!(script_bytes[0], 0x20);
        // After 32 bytes of pubkey, we should have CHECKSIG (0xac)
        assert_eq!(script_bytes[33], opcodes::all::OP_CHECKSIG.to_u8());
        // Then OP_FALSE (0x00)
        assert_eq!(script_bytes[34], opcodes::OP_FALSE.to_u8());
        // Then OP_IF (0x63)
        assert_eq!(script_bytes[35], opcodes::all::OP_IF.to_u8());
    }

    #[test]
    fn test_call_envelope_script() {
        let json = r#"{"p":"brc20-prog","op":"call","c":"0x1234567890abcdef","d":"0xa9059cbb"}"#;
        let envelope = Brc20ProgEnvelope::for_call(json.to_string());
        let pubkey = test_pubkey();
        let script = envelope.build_reveal_script(pubkey);

        // Verify script contains the JSON payload
        let script_bytes = script.as_bytes();
        let script_str = String::from_utf8_lossy(script_bytes);
        assert!(script_str.contains("brc20-prog"));
        assert!(script_str.contains("call"));
    }
}
