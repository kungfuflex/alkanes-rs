// Regression tests for the named-rune commitment predicate. These call
// `check_rune_etch_commitment` directly because the `cfg(test)`
// `validate_rune_etch` shim accepts every etching.
#[cfg(test)]
mod tests {
    use crate::test_helpers::{self as helpers, clear};
    use crate::{check_rune_etch_commitment, tables, Protorune};
    use bitcoin::hashes::Hash;
    use bitcoin::key::{Secp256k1, UntweakedPublicKey};
    use bitcoin::opcodes::all::OP_DROP;
    use bitcoin::script::{Builder, PushBytesBuf};
    use bitcoin::{
        Amount, OutPoint, PubkeyHash, ScriptBuf, ScriptHash, Transaction, TxOut, WPubkeyHash,
        Witness,
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use ordinals::Rune;
    use protorune_support::utils::consensus_encode;
    use std::str::FromStr;
    use std::sync::Arc;
    use wasm_bindgen_test::wasm_bindgen_test;

    const COMMIT_HEIGHT: u64 = 840_000;

    fn commitment() -> Vec<u8> {
        Rune::from_str("AAAAAAAAAAAAATESTER").unwrap().commitment()
    }

    fn p2tr() -> ScriptBuf {
        let secp = Secp256k1::verification_only();
        let key = UntweakedPublicKey::from_str(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        ScriptBuf::new_p2tr(&secp, key, None)
    }

    fn p2wsh() -> ScriptBuf {
        Builder::new()
            .push_opcode(OP_DROP)
            .push_int(1)
            .into_script()
            .to_p2wsh()
    }

    fn p2wpkh() -> ScriptBuf {
        ScriptBuf::new_p2wpkh(&WPubkeyHash::all_zeros())
    }

    fn p2sh() -> ScriptBuf {
        ScriptBuf::new_p2sh(&ScriptHash::all_zeros())
    }

    fn p2pkh() -> ScriptBuf {
        ScriptBuf::new_p2pkh(&PubkeyHash::all_zeros())
    }

    /// Indexes a block at `height` with one output paying `script_pubkey` and
    /// returns that outpoint.
    fn fund(script_pubkey: ScriptBuf, height: u64) -> OutPoint {
        let funding = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![helpers::get_mock_txin(0)],
            output: vec![TxOut {
                value: Amount::from_sat(10_000),
                script_pubkey,
            }],
        };
        Protorune::index_outpoints(&helpers::create_block_with_txs(vec![funding.clone()]), height)
            .unwrap();
        OutPoint {
            txid: funding.compute_txid(),
            vout: 0,
        }
    }

    /// A script-path shaped witness: [leaf script pushing `pushed`, control block].
    fn script_path_witness(pushed: &[u8]) -> Witness {
        let leaf = Builder::new()
            .push_slice(PushBytesBuf::try_from(pushed.to_vec()).unwrap())
            .into_script();
        let mut control_block = vec![0xc0];
        control_block.extend_from_slice(&[0x02; 32]);
        Witness::from_slice(&[leaf.as_bytes().to_vec(), control_block])
    }

    fn spend(outpoint: OutPoint, witness: Witness) -> Transaction {
        let mut input = helpers::get_txin_from_outpoint(outpoint);
        input.witness = witness;
        Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![input],
            output: vec![TxOut {
                value: Amount::from_sat(1_000),
                script_pubkey: p2tr(),
            }],
        }
    }

    fn check(tx: &Transaction, confirmations: u64) -> bool {
        check_rune_etch_commitment(tx, &commitment(), COMMIT_HEIGHT + confirmations - 1, true)
            .unwrap()
    }

    #[wasm_bindgen_test]
    fn p2tr_script_path_commitment_needs_six_confirmations() {
        clear();
        let tx = spend(fund(p2tr(), COMMIT_HEIGHT), script_path_witness(&commitment()));
        assert!(!check(&tx, 5));
        assert!(check(&tx, 6));
    }

    #[wasm_bindgen_test]
    fn non_p2tr_outputs_are_rejected() {
        for script in [p2wsh(), p2wpkh(), p2sh(), p2pkh()] {
            clear();
            let tx = spend(fund(script.clone(), COMMIT_HEIGHT), script_path_witness(&commitment()));
            assert!(!check(&tx, 6), "accepted commitment from {:?}", script);
        }
    }

    #[wasm_bindgen_test]
    fn legacy_predicate_still_accepts_p2wsh_below_activation() {
        clear();
        let tx = spend(fund(p2wsh(), COMMIT_HEIGHT), script_path_witness(&commitment()));
        assert!(check_rune_etch_commitment(&tx, &commitment(), COMMIT_HEIGHT + 5, false).unwrap());
    }

    #[wasm_bindgen_test]
    fn key_path_spend_is_rejected() {
        clear();
        let tx = spend(
            fund(p2tr(), COMMIT_HEIGHT),
            Witness::from_slice(&[vec![0x01; 64]]),
        );
        assert!(!check(&tx, 6));
    }

    #[wasm_bindgen_test]
    fn wrong_commitment_is_rejected() {
        clear();
        let tx = spend(fund(p2tr(), COMMIT_HEIGHT), script_path_witness(&[0x42; 8]));
        assert!(!check(&tx, 6));
    }

    #[wasm_bindgen_test]
    fn unknown_outpoint_is_rejected() {
        clear();
        let outpoint = OutPoint {
            txid: bitcoin::Txid::all_zeros(),
            vout: 7,
        };
        let tx = spend(outpoint, script_path_witness(&commitment()));
        assert!(!check(&tx, 6));
        // the legacy predicate reads height 0 for unknown outpoints and accepts
        assert!(check_rune_etch_commitment(&tx, &commitment(), COMMIT_HEIGHT, false).unwrap());
    }

    #[wasm_bindgen_test]
    fn malformed_stored_output_is_rejected() {
        clear();
        let outpoint = fund(p2tr(), COMMIT_HEIGHT);
        tables::OUTPOINT_TO_OUTPUT
            .select(&consensus_encode(&outpoint).unwrap())
            .set(Arc::new(vec![0xff, 0xff, 0xff]));
        let tx = spend(outpoint, script_path_witness(&commitment()));
        assert!(!check(&tx, 6));
    }
}
