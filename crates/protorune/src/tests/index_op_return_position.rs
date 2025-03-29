#[cfg(test)]
mod tests {
    use crate::balance_sheet::{load_sheet, PersistentRecord};
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::test_helpers::{self as helpers, get_address, ADDRESS1};
    use crate::{tables, Protorune};
    use anyhow::{anyhow, Result};
    use bitcoin::{OutPoint, Transaction};
    use metashrew::index_pointer::AtomicPointer;
    use metashrew::stdio::{stdout, Write};
    use metashrew_support::index_pointer::KeyValuePointer;
    use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
    use protorune_support::rune_transfer::RuneTransfer;
    use protorune_support::utils::consensus_encode;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    use helpers::clear;

    // Define a NoopMessageContext that doesn't do anything special with the protorunes
    struct NoopMessageContext;

    impl MessageContext for NoopMessageContext {
        fn protocol_tag() -> u128 {
            122 // Using the same protocol tag as in the tests
        }

        fn handle(
            parcel: &MessageContextParcel,
        ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
            // Just return the runes as-is without any special handling
            let runes: Vec<RuneTransfer> = parcel.runes.clone();
            Ok((runes, BalanceSheet::default()))
        }
    }

    // Helper function to create a transaction with OP_RETURN in the middle
    fn create_tx_with_middle_op_return(protocol_id: u128) -> Transaction {
        let first_mock_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };

        // Create a transaction with OP_RETURN in the middle (output index 1)
        // Output 0: Normal output
        // Output 1: OP_RETURN with runestone
        // Output 2: Normal output
        helpers::create_transaction_with_middle_op_return(first_mock_output, protocol_id)
    }

    // Helper function to create a block with a transaction that has OP_RETURN in the middle
    fn create_block_with_middle_op_return(protocol_id: u128) -> bitcoin::Block {
        let tx = create_tx_with_middle_op_return(protocol_id);
        helpers::create_block_with_txs(vec![tx])
    }

    // Helper function to create a transaction with OP_RETURN at the end
    fn create_tx_with_end_op_return(protocol_id: u128) -> Transaction {
        let first_mock_output = OutPoint {
            txid: bitcoin::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        };

        // Create a protoburn transaction that sends protorunes to ADDRESS1
        let protoburn_tx =
            helpers::create_default_protoburn_transaction(first_mock_output, protocol_id);

        protoburn_tx
    }

    // Helper function to create a block with a transaction that has OP_RETURN at the end
    fn create_block_with_end_op_return(protocol_id: u128) -> bitcoin::Block {
        let tx = create_tx_with_end_op_return(protocol_id);
        helpers::create_block_with_txs(vec![tx])
    }

    // Test that protorunes are correctly indexed when OP_RETURN is at the end
    #[wasm_bindgen_test]
    fn test_op_return_not_last() -> Result<()> {
        clear();
        let block_height = 840000;
        let protocol_id = 122;

        // Test with OP_RETURN at the end (should work)
        let test_block_end = create_block_with_end_op_return(protocol_id);
        assert!(
            Protorune::index_block::<NoopMessageContext>(test_block_end.clone(), block_height)
                .is_ok()
        );

        // Check that protorunes are correctly indexed for the output
        let tx_end = &test_block_end.txdata[0];
        let txid_end = tx_end.compute_txid();

        // Check output 0 (before OP_RETURN)
        let outpoint_end = OutPoint {
            txid: txid_end.clone(),
            vout: 0,
        };
        let sheet_end = load_sheet(
            &tables::RuneTable::for_protocol(protocol_id)
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&outpoint_end).unwrap()),
        );

        // The output should have protorunes
        let protorune_id = ProtoruneRuneId {
            block: block_height as u128,
            tx: 0,
        };

        // Print debug information
        println!("Protocol ID: {}", protocol_id);
        println!("Protorune ID: {:?}", protorune_id);
        println!("Sheet balance: {}", sheet_end.get_cached(&protorune_id));

        let has_protorunes_end = sheet_end.get_cached(&protorune_id) > 0;
        assert!(
            has_protorunes_end,
            "Expected protorunes when OP_RETURN is at the end"
        );

        Ok(())
    }
}
