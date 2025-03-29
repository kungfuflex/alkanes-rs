#[cfg(test)]
mod tests {
    use crate::message::{MessageContext, MessageContextParcel};
    use crate::test_helpers::{self as helpers};
    use crate::{view, Protorune};
    use anyhow::Result;
    use bitcoin::OutPoint;
    use metashrew::index_pointer::AtomicPointer;
    use metashrew::{
        println,
        stdio::{stdout, Write},
    };
    use protobuf::{Message, MessageField};
    use protorune_support::balance_sheet::BalanceSheet;
    use protorune_support::proto::protorune::{ProtorunesWalletRequest, WalletResponse};
    use protorune_support::rune_transfer::RuneTransfer;
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

    // Helper function to create a transaction with OP_RETURN at the end
    fn create_tx_with_end_op_return(protocol_id: u128) -> bitcoin::Transaction {
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

    // Test that protorunes_by_address returns all protorunes for a given address
    #[wasm_bindgen_test]
    fn test_protorunes_by_address() -> Result<()> {
        clear();
        let block_height = 840000;
        let protocol_id = 122;

        // Create and index a block with a transaction that has OP_RETURN at the end
        let test_block = create_block_with_end_op_return(protocol_id);
        assert!(
            Protorune::index_block::<NoopMessageContext>(test_block.clone(), block_height).is_ok()
        );

        // Get the address from the transaction
        let tx = &test_block.txdata[0];
        let address = helpers::get_address(&helpers::ADDRESS1().as_str());
        let address_bytes = address.to_string().into_bytes();

        // Create a request to get protorunes for the address
        let mut request = ProtorunesWalletRequest::new();
        request.wallet = address_bytes.clone();
        request.protocol_tag = MessageField::some(protocol_id.into());

        // Call protorunes_by_address
        let response: WalletResponse =
            view::protorunes_by_address2(&request.write_to_bytes().unwrap())?;

        println!("Response outpoints count: {}", response.outpoints.len());

        // If there are outpoints, print some information about them

        assert!(
            response.outpoints.len() > 0,
            "must return at least one outpoint"
        );

        Ok(())
    }
}
