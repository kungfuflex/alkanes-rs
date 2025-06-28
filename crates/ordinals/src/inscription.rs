//! Unified inscription interface for Bitcoin and Dogecoin
//! 
//! This module provides a feature-gated interface that uses either:
//! - Bitcoin: Taproot witness-based inscriptions (single transaction)
//! - Dogecoin: Script_sig-based inscriptions (multi-transaction support)

use bitcoin::Transaction;

#[cfg(feature = "dogecoin")]
pub use ordinals_scriptsig::{Inscription, ParsedInscription};

#[cfg(not(feature = "dogecoin"))]
pub use bitcoin_inscription::{Inscription, ParsedInscription};

/// Parse inscriptions from transactions using the appropriate method for the chain
pub fn parse_inscriptions_from_transactions(txs: Vec<Transaction>) -> ParsedInscription {
    #[cfg(feature = "dogecoin")]
    {
        // Use Dogecoin script_sig parsing with multi-transaction support
        Inscription::from_transactions(txs)
    }
    
    #[cfg(not(feature = "dogecoin"))]
    {
        // Use Bitcoin taproot witness parsing (single transaction)
        if txs.is_empty() {
            return ParsedInscription::None;
        }
        
        // For Bitcoin, we only parse the first transaction's witness
        bitcoin_inscription::parse_from_witness(&txs[0])
    }
}

#[cfg(not(feature = "dogecoin"))]
mod bitcoin_inscription {
    use super::*;
    use bitcoin::{
        blockdata::script::Instruction,
        Script,
    };

    #[derive(Debug, PartialEq, Clone)]
    pub struct Inscription {
        body: Option<Vec<u8>>,
        content_type: Option<Vec<u8>>,
    }

    #[derive(Debug, PartialEq)]
    pub enum ParsedInscription {
        None,
        Partial,
        Complete(Inscription),
    }

    impl Inscription {
        pub fn new(content_type: Option<Vec<u8>>, body: Option<Vec<u8>>) -> Self {
            Self { content_type, body }
        }

        pub fn body(&self) -> Option<&[u8]> {
            Some(self.body.as_ref()?)
        }

        pub fn into_body(self) -> Option<Vec<u8>> {
            self.body
        }

        pub fn content_length(&self) -> Option<usize> {
            Some(self.body()?.len())
        }

        pub fn content_type(&self) -> Option<&str> {
            std::str::from_utf8(self.content_type.as_ref()?).ok()
        }
    }

    /// Parse inscription from Bitcoin taproot witness
    pub fn parse_from_witness(tx: &Transaction) -> ParsedInscription {
        for input in &tx.input {
            if let Some(tapscript) = input.witness.tapscript() {
                if let Ok(inscription) = parse_from_tapscript(tapscript) {
                    return inscription;
                }
            }
        }
        ParsedInscription::None
    }

    fn parse_from_tapscript(tapscript: &Script) -> Result<ParsedInscription, ()> {
        let mut instructions = tapscript.instructions().peekable();
        
        // Look for OP_FALSE OP_IF pattern
        while let Some(instruction) = instructions.next() {
            if let Ok(Instruction::PushBytes(push_bytes)) = instruction {
                if push_bytes.is_empty() {
                    // Found OP_FALSE, check for OP_IF
                    if let Some(Ok(Instruction::Op(bitcoin::opcodes::all::OP_IF))) = instructions.peek() {
                        instructions.next(); // consume OP_IF
                        return parse_inscription_content(&mut instructions);
                    }
                }
            }
        }
        
        Ok(ParsedInscription::None)
    }

    fn parse_inscription_content(
        instructions: &mut std::iter::Peekable<bitcoin::script::Instructions>
    ) -> Result<ParsedInscription, ()> {
        let mut content_type: Option<Vec<u8>> = None;
        let mut body: Option<Vec<u8>> = None;
        
        // Look for protocol identifier "ord"
        if let Some(Ok(Instruction::PushBytes(push_bytes))) = instructions.next() {
            if push_bytes.as_bytes() != b"ord" {
                return Ok(ParsedInscription::None);
            }
        } else {
            return Ok(ParsedInscription::None);
        }
        
        // Parse fields
        while let Some(instruction) = instructions.next() {
            match instruction {
                Ok(Instruction::Op(bitcoin::opcodes::all::OP_ENDIF)) => break,
                Ok(Instruction::PushBytes(tag)) => {
                    if tag.len() == 1 {
                        match tag.as_bytes()[0] {
                            1 => {
                                // Content type field
                                if let Some(Ok(Instruction::PushBytes(ct))) = instructions.next() {
                                    content_type = Some(ct.as_bytes().to_vec());
                                }
                            }
                            0 => {
                                // Body field - collect all remaining pushes until OP_ENDIF
                                let mut body_parts = Vec::new();
                                while let Some(instruction) = instructions.next() {
                                    match instruction {
                                        Ok(Instruction::Op(bitcoin::opcodes::all::OP_ENDIF)) => {
                                            body = Some(body_parts.concat());
                                            return Ok(ParsedInscription::Complete(Inscription {
                                                content_type,
                                                body,
                                            }));
                                        }
                                        Ok(Instruction::PushBytes(data)) => {
                                            body_parts.push(data.as_bytes().to_vec());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {
                                // Skip unknown fields
                                instructions.next();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        if content_type.is_some() || body.is_some() {
            Ok(ParsedInscription::Complete(Inscription {
                content_type,
                body,
            }))
        } else {
            Ok(ParsedInscription::None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{TxIn, TxOut, OutPoint, Sequence, Witness};

    fn create_test_transaction() -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version(1),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: bitcoin::Script::new().into(),
                sequence: Sequence::ZERO,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: bitcoin::Script::new().into(),
            }],
        }
    }

    #[test]
    fn test_empty_transactions() {
        assert_eq!(
            parse_inscriptions_from_transactions(vec![]),
            ParsedInscription::None
        );
    }

    #[test]
    fn test_transaction_without_inscription() {
        let tx = create_test_transaction();
        assert_eq!(
            parse_inscriptions_from_transactions(vec![tx]),
            ParsedInscription::None
        );
    }

    #[cfg(feature = "dogecoin")]
    #[test]
    fn test_dogecoin_feature_enabled() {
        // When dogecoin feature is enabled, we should use ordinals-scriptsig
        let tx = create_test_transaction();
        let result = parse_inscriptions_from_transactions(vec![tx]);
        // This will use the Dogecoin script_sig parsing logic
        assert_eq!(result, ParsedInscription::None);
    }
}