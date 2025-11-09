// BRC20-Prog envelope implementation using ord inscription format
// Unlike alkanes (BIN protocol), this uses the standard ord envelope with text/plain content-type

use crate::AlkanesError;
use alloc::format;
use anyhow::Result;
use bitcoin::{
    blockdata::opcodes,
    script::Builder as ScriptBuilder,
    taproot::ControlBlock,
    ScriptBuf, Witness,
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
    /// Structure: OP_FALSE OP_IF "ord" <content_type_tag> "text/plain" <body_tag> <payload> OP_ENDIF
    pub fn build_reveal_script(&self) -> ScriptBuf {
        let mut builder = ScriptBuilder::new()
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
    ) -> Result<Witness, AlkanesError> {
        let reveal_script = self.build_reveal_script();

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

    #[test]
    fn test_deploy_envelope_script() {
        let json = r#"{"p":"brc20-prog","op":"deploy","d":"0x608060405234801561001057600080fd5b50"}"#;
        let envelope = Brc20ProgEnvelope::for_deploy(json.to_string());
        let script = envelope.build_reveal_script();

        // Verify script starts with OP_FALSE OP_IF "ord"
        let script_bytes = script.as_bytes();
        assert!(script_bytes.len() > 10);
        assert_eq!(script_bytes[0], opcodes::OP_FALSE.to_u8());
        assert_eq!(script_bytes[1], opcodes::all::OP_IF.to_u8());
    }

    #[test]
    fn test_call_envelope_script() {
        let json = r#"{"p":"brc20-prog","op":"call","c":"0x1234567890abcdef","d":"0xa9059cbb"}"#;
        let envelope = Brc20ProgEnvelope::for_call(json.to_string());
        let script = envelope.build_reveal_script();

        // Verify script contains the JSON payload
        let script_bytes = script.as_bytes();
        let script_str = String::from_utf8_lossy(script_bytes);
        assert!(script_str.contains("brc20-prog"));
        assert!(script_str.contains("call"));
    }
}
