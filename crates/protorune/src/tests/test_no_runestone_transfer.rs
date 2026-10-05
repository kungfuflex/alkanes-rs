//! A transaction with no Runestone must still move its input runes to the first
//! non-OP_RETURN output (ord's default), and burn them only when every output is
//! OP_RETURN. Before the fix the inputs were cleared and nothing was credited.
#[cfg(test)]
mod tests {
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::test_helpers::{self as helpers, clear, ADDRESS1, ADDRESS2};
    use crate::Protorune;
    use anyhow::Result;
    use bitcoin::{Amount, OutPoint, ScriptBuf, Transaction, TxIn, TxOut};
    use metashrew_core::index_pointer::AtomicPointer;
    use ordinals::{Etching, Rune, Runestone};
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::rune_transfer::RuneTransfer;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    const HEIGHT: u64 = 840_000;

    struct NoopContext;

    impl MessageContext for NoopContext {
        fn handle(
            _parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            Ok((vec![], BalanceSheet::default()))
        }

        fn protocol_tag() -> u128 {
            100
        }
    }

    fn rid(tx: u128) -> ProtoruneRuneId {
        ProtoruneRuneId::new(HEIGHT as u128, tx)
    }

    fn etch_tx(name: &str, mock_input: u32, premine: u128) -> Transaction {
        helpers::create_tx_from_runestone(
            Runestone {
                etching: Some(Etching {
                    divisibility: Some(0),
                    premine: Some(premine),
                    rune: Some(Rune::from_str(name).unwrap()),
                    spacers: Some(0),
                    symbol: Some('C'),
                    turbo: true,
                    terms: None,
                }),
                pointer: Some(0),
                ..Default::default()
            },
            vec![helpers::get_mock_txin(mock_input)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS1(), 1_000)],
        )
    }

    /// Etches rune (HEIGHT, 0) with 1_000 units and rune (HEIGHT, 1) with 300 units,
    /// each landing on vout 0 of its etching tx.
    fn setup() -> (Transaction, Transaction) {
        clear();
        let a = etch_tx("AAAAAAAAAAAAANORUNESTONE", 0, 1_000);
        let b = etch_tx("BBBBBBBBBBBBBNORUNESTONE", 1, 300);
        Protorune::index_block::<NoopContext>(
            helpers::create_block_with_txs(vec![a.clone(), b.clone()]),
            HEIGHT,
        )
        .unwrap();
        assert_eq!(balances(&a, 0), vec![1_000, 0]);
        assert_eq!(balances(&b, 0), vec![0, 300]);
        (a, b)
    }

    fn balances(tx: &Transaction, vout: u32) -> Vec<u128> {
        helpers::get_rune_balance_by_outpoint(
            OutPoint {
                txid: tx.compute_txid(),
                vout,
            },
            vec![rid(0), rid(1)],
        )
    }

    fn spend_of(tx: &Transaction) -> TxIn {
        helpers::get_txin_from_outpoint(OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        })
    }

    /// An OP_RETURN that is not a Runestone (no OP_13 tag).
    fn plain_op_return() -> TxOut {
        TxOut {
            value: Amount::from_sat(0),
            script_pubkey: ScriptBuf::new_op_return(&[0xde, 0xad]),
        }
    }

    fn plain_tx(input: Vec<TxIn>, output: Vec<TxOut>) -> Transaction {
        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input,
            output,
        };
        assert!(Runestone::decipher(&tx).is_none());
        tx
    }

    fn index(tx: &Transaction) {
        Protorune::index_block::<NoopContext>(
            helpers::create_block_with_txs(vec![tx.clone()]),
            HEIGHT + 1,
        )
        .unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn no_runestone_moves_balance_to_first_output() {
        let (a, _) = setup();
        let spend = plain_tx(
            vec![spend_of(&a)],
            vec![
                helpers::get_txout_transfer_to_address(&ADDRESS2(), 500),
                helpers::get_txout_transfer_to_address(&ADDRESS1(), 400),
            ],
        );
        index(&spend);
        assert_eq!(balances(&a, 0), vec![0, 0]);
        assert_eq!(balances(&spend, 0), vec![1_000, 0]);
        assert_eq!(balances(&spend, 1), vec![0, 0]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn no_runestone_merges_multiple_inputs_and_rune_ids() {
        let (a, b) = setup();
        // a and b carry different runes; the mock input carries none.
        let spend = plain_tx(
            vec![spend_of(&a), helpers::get_mock_txin(7), spend_of(&b)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS2(), 500)],
        );
        index(&spend);
        assert_eq!(balances(&a, 0), vec![0, 0]);
        assert_eq!(balances(&b, 0), vec![0, 0]);
        assert_eq!(balances(&spend, 0), vec![1_000, 300]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn no_runestone_skips_leading_op_return() {
        let (a, _) = setup();
        let spend = plain_tx(
            vec![spend_of(&a)],
            vec![
                plain_op_return(),
                helpers::get_txout_transfer_to_address(&ADDRESS2(), 500),
                helpers::get_txout_transfer_to_address(&ADDRESS1(), 400),
            ],
        );
        index(&spend);
        assert_eq!(balances(&spend, 0), vec![0, 0]);
        assert_eq!(balances(&spend, 1), vec![1_000, 0]);
        assert_eq!(balances(&spend, 2), vec![0, 0]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn no_runestone_burns_when_only_op_returns() {
        let (a, _) = setup();
        let spend = plain_tx(vec![spend_of(&a)], vec![plain_op_return()]);
        index(&spend);
        assert_eq!(balances(&a, 0), vec![0, 0]);
        assert_eq!(balances(&spend, 0), vec![0, 0]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn empty_runestone_control_matches_no_runestone() {
        let (a, _) = setup();
        let spend = helpers::create_tx_from_runestone(
            Runestone::default(),
            vec![spend_of(&a)],
            vec![helpers::get_txout_transfer_to_address(&ADDRESS2(), 500)],
        );
        index(&spend);
        assert_eq!(balances(&a, 0), vec![0, 0]);
        assert_eq!(balances(&spend, 0), vec![1_000, 0]);
    }
}
