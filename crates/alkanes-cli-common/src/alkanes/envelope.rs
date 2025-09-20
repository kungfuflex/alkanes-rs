// Alkanes envelope implementation based on alkanes-rs reference
// Core functionality for creating and managing alkanes envelope transactions
// CRITICAL FIX: Updated to match alkanes-rs reference implementation exactly
// Key differences: uses gzip compression, no content-type tags, proper BIN protocol structure

use crate::DeezelError;
use alloc::format;
use anyhow::Result;
use bitcoin::{
    blockdata::opcodes,
    script::Builder as ScriptBuilder,
    taproot::ControlBlock, ScriptBuf, Witness,
};
use flate2::{write::GzEncoder, Compression};
#[cfg(feature = "std")]
use std::io::Write;

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

// Alkanes protocol constants - matching alkanes-rs reference exactly
pub const ALKANES_PROTOCOL_ID: [u8; 3] = *b"BIN";
pub const BODY_TAG: [u8; 0] = [];
const MAX_SCRIPT_ELEMENT_SIZE: usize = 520;

/// Alkanes envelope structure for contract deployment
/// CRITICAL FIX: Simplified to match alkanes-rs reference - no content-type field
#[derive(Debug, Clone)]
pub struct AlkanesEnvelope {
    pub payload: Vec<u8>,
}

impl AlkanesEnvelope {
    /// Create new alkanes envelope with contract data
    /// CRITICAL FIX: Simplified constructor to match alkanes-rs reference
    pub fn new(payload: Vec<u8>) -> Self {
        Self { payload }
    }

    /// Create envelope for alkanes contract deployment with BIN protocol data
    /// This envelope will be used as the first input in the reveal transaction
    pub fn for_contract(contract_data: Vec<u8>) -> Self {
        Self::new(contract_data)
    }

    /// Compress payload using gzip compression (matching alkanes-rs reference)
    /// CRITICAL FIX: Added gzip compression like alkanes-rs reference
    #[cfg(feature = "std")]
    fn compress_payload(&self) -> Result<Vec<u8>, DeezelError> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.payload)
            .map_err(|e| DeezelError::Other(format!("Failed to write payload to gzip encoder: {e}")))?;
        encoder.finish()
            .map_err(|e| DeezelError::Other(format!("Failed to finish gzip compression: {e}")))
    }

    #[cfg(not(feature = "std"))]
    fn compress_payload(&self) -> Result<Vec<u8>, DeezelError> {
        // This is a temporary workaround for no_std builds.
        // A full no_std compression implementation will be added later.
        Ok(self.payload.clone())
    }

    /// Build the reveal script following alkanes-rs reference EXACTLY
    /// CRITICAL FIX: Match alkanes-rs reference implementation exactly
    pub fn build_reveal_script(&self) -> ScriptBuf {
        let mut builder = ScriptBuilder::new()
            .push_opcode(opcodes::OP_FALSE) // OP_FALSE (pushes empty bytes)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(ALKANES_PROTOCOL_ID); // BIN protocol ID

        // CRITICAL FIX: Add empty BODY_TAG before compressed payload (matching alkanes-rs reference)
        builder = builder.push_slice(BODY_TAG);

        // CRITICAL FIX: Compress the payload using gzip (matching alkanes-rs reference)
        if let Ok(compressed_payload) = self.compress_payload() {
            // Chunk compressed data into script-safe pieces
            for chunk in compressed_payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
                builder = builder.push_slice::<&bitcoin::script::PushBytes>(chunk.try_into().unwrap());
            }
        } else {
            log::warn!("Failed to compress payload, using uncompressed data");
            // Fallback to uncompressed data
            for chunk in self.payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
                builder = builder.push_slice::<&bitcoin::script::PushBytes>(chunk.try_into().unwrap());
            }
        }

        // End with OP_ENDIF
        builder
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script()
    }

    /// Create complete witness for taproot script-path spending with signature
    /// CRITICAL FIX: This creates the complete 3-element witness: [signature, script, control_block]
    /// This is what should be used for the final transaction
    pub fn create_complete_witness(&self, signature: &[u8], control_block: ControlBlock) -> Result<Witness, DeezelError> {
        let reveal_script = self.build_reveal_script();
        
        let mut witness = Witness::new();
        
        // CRITICAL FIX: Create complete P2TR script-path witness structure
        // For P2TR script-path spending: [signature, script, control_block]
        
        // 1. Push the signature as the FIRST element
        witness.push(signature);
        
        // 2. Push the script bytes - this contains the BIN protocol envelope data
        let script_bytes = reveal_script.as_bytes();
        witness.push(script_bytes);
        
        // 3. Push the control block bytes
        let control_block_bytes = control_block.serialize();
        witness.push(&control_block_bytes);
        
        // Verify the witness was created correctly - expecting 3 items for complete P2TR
        if witness.len() != 3 {
            return Err(DeezelError::Other(format!("Invalid complete witness length: expected 3 items (signature + script + control_block), got {}", witness.len())));
        }
        
        // Verify all elements are non-empty
        for (i, item) in witness.iter().enumerate() {
            if item.is_empty() {
                return Err(DeezelError::Other(format!("Witness item {i} is empty")));
            }
        }
        
        Ok(witness)
    }
}
