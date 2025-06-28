//! Feature-gated tests for inscription parsing
//! 
//! This module contains tests that verify both Bitcoin and Dogecoin inscription parsing
//! work correctly depending on the feature flags enabled.

#[cfg(test)]
mod tests {
    use super::super::inscription::*;
    use bitcoin::{
        blockdata::script,
        opcodes::{self, all::*},
        Transaction, TxIn, TxOut, OutPoint, Sequence, Witness, Script,
    };

    fn create_basic_transaction() -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version(1),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: Script::new().into(),
                sequence: Sequence::ZERO,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::ZERO,
                script_pubkey: Script::new().into(),
            }],
        }
    }

    #[test]
    fn test_empty_transactions() {
        let result = parse_inscriptions_from_transactions(vec![]);
        assert_eq!(result, ParsedInscription::None);
    }

    #[test]
    fn test_transaction_without_inscription() {
        let tx = create_basic_transaction();
        let result = parse_inscriptions_from_transactions(vec![tx]);
        assert_eq!(result, ParsedInscription::None);
    }

    #[cfg(feature = "dogecoin")]
    mod dogecoin_tests {
        use super::*;

        fn create_dogecoin_inscription_transaction(
            content_type: &str,
            body: &str,
            npieces: u8,
        ) -> Transaction {
            let mut script_data: Vec<u8> = Vec::new();
            
            // Protocol identifier
            script_data.push(3); // length of "ord"
            script_data.extend_from_slice(b"ord");
            
            // Number of pieces
            script_data.push(80 + npieces); // OP_1, OP_2, etc.
            
            // Content type
            script_data.push(content_type.len() as u8);
            script_data.extend_from_slice(content_type.as_bytes());
            
            // Body with countdown
            script_data.push(0); // countdown = 0 (last piece)
            script_data.push(body.len() as u8);
            script_data.extend_from_slice(body.as_bytes());

            let mut tx = create_basic_transaction();
            tx.input[0].script_sig = Script::from(script_data);
            tx
        }

        #[test]
        fn test_dogecoin_single_transaction_inscription() {
            let tx = create_dogecoin_inscription_transaction("text/plain", "hello", 1);
            let result = parse_inscriptions_from_transactions(vec![tx]);
            
            match result {
                ParsedInscription::Complete(inscription) => {
                    assert_eq!(inscription.content_type(), Some("text/plain"));
                    assert_eq!(inscription.body(), Some(b"hello"));
                }
                _ => panic!("Expected complete inscription"),
            }
        }

        #[test]
        fn test_dogecoin_multi_transaction_inscription() {
            // First transaction with first part
            let mut script1_data: Vec<u8> = Vec::new();
            script1_data.push(3); // "ord"
            script1_data.extend_from_slice(b"ord");
            script1_data.push(82); // OP_2 (2 pieces)
            script1_data.push(10); // content type length
            script1_data.extend_from_slice(b"text/plain");
            script1_data.push(81); // countdown = 1
            script1_data.push(5); // body part length
            script1_data.extend_from_slice(b"hello");

            let mut tx1 = create_basic_transaction();
            tx1.input[0].script_sig = Script::from(script1_data);

            // Second transaction with second part
            let mut script2_data: Vec<u8> = Vec::new();
            script2_data.push(0); // countdown = 0
            script2_data.push(6); // body part length
            script2_data.extend_from_slice(b" world");

            let mut tx2 = create_basic_transaction();
            tx2.input[0].script_sig = Script::from(script2_data);

            let result = parse_inscriptions_from_transactions(vec![tx1, tx2]);
            
            match result {
                ParsedInscription::Complete(inscription) => {
                    assert_eq!(inscription.content_type(), Some("text/plain"));
                    assert_eq!(inscription.body(), Some(b"hello world"));
                }
                _ => panic!("Expected complete inscription, got {:?}", result),
            }
        }

        #[test]
        fn test_dogecoin_wrong_protocol() {
            let mut script_data: Vec<u8> = Vec::new();
            script_data.push(3);
            script_data.extend_from_slice(b"dog"); // wrong protocol
            script_data.push(81);
            script_data.push(10);
            script_data.extend_from_slice(b"text/plain");
            script_data.push(0);
            script_data.push(5);
            script_data.extend_from_slice(b"hello");

            let mut tx = create_basic_transaction();
            tx.input[0].script_sig = Script::from(script_data);
            
            let result = parse_inscriptions_from_transactions(vec![tx]);
            assert_eq!(result, ParsedInscription::None);
        }

        #[test]
        fn test_dogecoin_incomplete_multipart() {
            // Create a transaction that expects 2 parts but only provide 1
            let mut script_data: Vec<u8> = Vec::new();
            script_data.push(3);
            script_data.extend_from_slice(b"ord");
            script_data.push(82); // OP_2 (expects 2 pieces)
            script_data.push(10);
            script_data.extend_from_slice(b"text/plain");
            script_data.push(81); // countdown = 1
            script_data.push(5);
            script_data.extend_from_slice(b"hello");

            let mut tx = create_basic_transaction();
            tx.input[0].script_sig = Script::from(script_data);
            
            let result = parse_inscriptions_from_transactions(vec![tx]);
            assert_eq!(result, ParsedInscription::Partial);
        }
    }

    #[cfg(not(feature = "dogecoin"))]
    mod bitcoin_tests {
        use super::*;

        fn create_bitcoin_inscription_witness(content_type: &str, body: &str) -> Witness {
            // Create script manually to avoid PushBytes issues
            let mut script_bytes = Vec::new();
            
            // OP_FALSE
            script_bytes.push(0x00);
            // OP_IF
            script_bytes.push(0x63);
            // "ord" - 3 bytes
            script_bytes.push(0x03);
            script_bytes.extend_from_slice(b"ord");
            // content type tag - 1 byte
            script_bytes.push(0x01);
            script_bytes.push(0x01);
            // content type
            script_bytes.push(content_type.len() as u8);
            script_bytes.extend_from_slice(content_type.as_bytes());
            // body tag - 1 byte
            script_bytes.push(0x01);
            script_bytes.push(0x00);
            // body
            script_bytes.push(body.len() as u8);
            script_bytes.extend_from_slice(body.as_bytes());
            // OP_ENDIF
            script_bytes.push(0x68);
            
            let script = bitcoin::ScriptBuf::from(script_bytes);

            let mut witness = Witness::new();
            witness.push(script);
            witness.push(&[]); // empty signature
            witness
        }

        #[test]
        fn test_bitcoin_taproot_inscription() {
            let mut tx = create_basic_transaction();
            tx.input[0].witness = create_bitcoin_inscription_witness("text/plain", "hello world");
            
            let result = parse_inscriptions_from_transactions(vec![tx]);
            
            match result {
                ParsedInscription::Complete(inscription) => {
                    assert_eq!(inscription.content_type(), Some("text/plain"));
                    assert_eq!(inscription.body(), Some(b"hello world".as_slice()));
                }
                _ => panic!("Expected complete inscription"),
            }
        }

        #[test]
        fn test_bitcoin_no_witness() {
            let tx = create_basic_transaction();
            let result = parse_inscriptions_from_transactions(vec![tx]);
            assert_eq!(result, ParsedInscription::None);
        }

        #[test]
        fn test_bitcoin_wrong_protocol() {
            let script = script::Builder::new()
                .push_opcode(opcodes::OP_FALSE)
                .push_opcode(OP_IF)
                .push_slice(b"dog") // wrong protocol
                .push_slice(&[1])
                .push_slice(b"text/plain")
                .push_slice(&[0])
                .push_slice(b"hello")
                .push_opcode(OP_ENDIF)
                .into_script();

            let mut witness = Witness::new();
            witness.push(script);
            witness.push(&[]);

            let mut tx = create_basic_transaction();
            tx.input[0].witness = witness;
            
            let result = parse_inscriptions_from_transactions(vec![tx]);
            assert_eq!(result, ParsedInscription::None);
        }
    }

    #[test]
    fn test_inscription_interface() {
        // Test that the Inscription interface works regardless of feature flags
        let inscription = Inscription::new(
            Some(b"text/plain".to_vec()),
            Some(b"test content".to_vec()),
        );

        assert_eq!(inscription.content_type(), Some("text/plain"));
        assert_eq!(inscription.body(), Some(b"test content".as_slice()));
        assert_eq!(inscription.content_length(), Some(12));

        let body = inscription.into_body();
        assert_eq!(body, Some(b"test content".to_vec()));
    }

    #[test]
    fn test_feature_flag_consistency() {
        // This test ensures that the feature flag behavior is consistent
        #[cfg(feature = "dogecoin")]
        {
            // When dogecoin feature is enabled, we should be using script_sig parsing
            println!("Testing with Dogecoin feature enabled - using script_sig parsing");
        }
        
        #[cfg(not(feature = "dogecoin"))]
        {
            // When dogecoin feature is disabled, we should be using taproot witness parsing
            println!("Testing with Bitcoin mode - using taproot witness parsing");
        }
    }
}