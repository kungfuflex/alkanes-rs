//! Dogecoin script_sig inscription parsing for ordinals
//! 
//! This crate provides inscription parsing functionality specifically for Dogecoin,
//! which uses script_sig fields instead of taproot witness scripts and supports
//! multi-transaction inscriptions with piece counting.

use thiserror::Error;

pub use inscription::{Inscription, ParsedInscription};

mod inscription;

#[derive(Error, Debug)]
pub enum InscriptionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid content type")]
    InvalidContentType,
    #[error("Content size limit exceeded")]
    ContentSizeExceeded,
}

/// Protocol identifier for ordinals inscriptions
pub const PROTOCOL_ID: &[u8] = b"ord";

/// Maximum size for a single script push operation (520 bytes)
pub const MAX_PUSH_SIZE: usize = 520;

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Transaction, TxIn, OutPoint, Sequence, Witness, ScriptBuf};

    fn inscription(content_type: &str, body: &str) -> Inscription {
        Inscription::new(
            Some(content_type.as_bytes().to_vec()),
            Some(body.as_bytes().to_vec()),
        )
    }

    #[test]
    fn test_empty_script() {
        assert_eq!(
            Inscription::from_transactions(vec![]),
            ParsedInscription::None
        );
    }

    #[test]
    fn test_valid_single_transaction() {
        let mut script: Vec<&[u8]> = Vec::new();
        script.push(&[3]);
        script.push(b"ord");
        script.push(&[81]); // OP_1 (npieces = 1)
        script.push(&[24]);
        script.push(b"text/plain;charset=utf-8");
        script.push(&[0]); // countdown = 0
        script.push(&[4]);
        script.push(b"woof");

        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from(script.concat()),
                sequence: Sequence::ZERO,
                witness: Witness::new(),
            }],
            output: Vec::new(),
        };

        assert_eq!(
            Inscription::from_transactions(vec![tx]),
            ParsedInscription::Complete(inscription("text/plain;charset=utf-8", "woof"))
        );
    }

    #[test]
    fn test_multi_transaction() {
        let mut script1: Vec<&[u8]> = Vec::new();
        let mut script2: Vec<&[u8]> = Vec::new();
        
        script1.push(&[3]);
        script1.push(b"ord");
        script1.push(&[82]); // OP_2 (npieces = 2)
        script1.push(&[24]);
        script1.push(b"text/plain;charset=utf-8");
        script1.push(&[81]); // countdown = 1
        script1.push(&[4]);
        script1.push(b"woof");
        
        script2.push(&[0]); // countdown = 0
        script2.push(&[5]);
        script2.push(b" woof");

        let tx1 = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from(script1.concat()),
                sequence: Sequence::ZERO,
                witness: Witness::new(),
            }],
            output: Vec::new(),
        };

        let tx2 = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from(script2.concat()),
                sequence: Sequence::ZERO,
                witness: Witness::new(),
            }],
            output: Vec::new(),
        };

        assert_eq!(
            Inscription::from_transactions(vec![tx1, tx2]),
            ParsedInscription::Complete(inscription("text/plain;charset=utf-8", "woof woof"))
        );
    }
}