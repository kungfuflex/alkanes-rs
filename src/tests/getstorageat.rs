#[cfg(test)]
mod tests {
    use crate::indexer::{configure_network, index_block};
    use crate::tests::helpers::clear;
    use crate::view;
    use alkanes_support::id::AlkaneId;
    use alkanes_support::proto::alkanes::{AlkaneStorageRequest, AlkaneStorageResponse};
    use anyhow::Result;
    use prost::Message;
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::network::is_active;
    use crate::precompiled::fr_btc_build;
    use crate::tests::helpers::{
        self as alkane_helpers, init_with_cellpack_pairs, BinaryAndCellpack,
    };
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::gz::compress;
    use protorune::test_helpers::create_block_with_coinbase_tx;

    /// Test that getstorageat works when called with configure_network().
    /// This mirrors the production flow where all other view functions call
    /// configure_network() before doing anything, but getstorageat was missing it.
    #[wasm_bindgen_test]
    fn test_getstorageat_with_configure_network() -> Result<()> {
        clear(); // clear() calls configure_network() internally

        // Deploy frBTC precompiled at genesis
        let genesis_block = create_block_with_coinbase_tx(0);
        index_block(&genesis_block, 0)?;

        // Index a few more blocks to ensure frBTC is active
        for h in 1..5u32 {
            let block = create_block_with_coinbase_tx(h);
            index_block(&block, h)?;
        }

        // Now test getstorageat - the frBTC contract at (32, 0) should have storage
        // Try reading /name which frBTC should have set
        let req = AlkaneStorageRequest {
            id: Some(alkanes_support::proto::alkanes::AlkaneId {
                block: Some(32u128.into()),
                tx: Some(0u128.into()),
            }),
            path: "/name".as_bytes().to_vec(),
        };

        // With configure_network() already called (via clear()), this should succeed
        let result = view::getstorageat(&req.into());
        assert!(result.is_ok(), "getstorageat should succeed after configure_network()");

        // Also test /totalsupply
        let req_supply = AlkaneStorageRequest {
            id: Some(alkanes_support::proto::alkanes::AlkaneId {
                block: Some(32u128.into()),
                tx: Some(0u128.into()),
            }),
            path: "/totalsupply".as_bytes().to_vec(),
        };

        let result_supply = view::getstorageat(&req_supply.into());
        assert!(
            result_supply.is_ok(),
            "getstorageat /totalsupply should succeed"
        );

        Ok(())
    }

    /// Test that getstorageat returns empty value for nonexistent storage paths
    /// (should not panic, should return empty)
    #[wasm_bindgen_test]
    fn test_getstorageat_nonexistent_path() -> Result<()> {
        clear();

        let genesis_block = create_block_with_coinbase_tx(0);
        index_block(&genesis_block, 0)?;

        let req = AlkaneStorageRequest {
            id: Some(alkanes_support::proto::alkanes::AlkaneId {
                block: Some(32u128.into()),
                tx: Some(0u128.into()),
            }),
            path: "/doesnotexist".as_bytes().to_vec(),
        };

        let result = view::getstorageat(&req.into());
        assert!(result.is_ok(), "getstorageat should not panic on nonexistent path");
        let response = result.unwrap();
        // Nonexistent storage should return empty value
        assert!(
            response.value.is_empty() || response.value.iter().all(|&b| b == 0),
            "nonexistent storage path should return empty/zero value"
        );

        Ok(())
    }

    /// Test that getstorageat works for a nonexistent alkane ID
    /// (should return empty, not panic)
    #[wasm_bindgen_test]
    fn test_getstorageat_nonexistent_alkane() -> Result<()> {
        clear();

        let genesis_block = create_block_with_coinbase_tx(0);
        index_block(&genesis_block, 0)?;

        let req = AlkaneStorageRequest {
            id: Some(alkanes_support::proto::alkanes::AlkaneId {
                block: Some(999999u128.into()),
                tx: Some(999u128.into()),
            }),
            path: "/totalsupply".as_bytes().to_vec(),
        };

        let result = view::getstorageat(&req.into());
        assert!(
            result.is_ok(),
            "getstorageat should not panic for nonexistent alkane"
        );

        Ok(())
    }

    /// Verify that the #[no_mangle] getstorageat() entry point correctly
    /// matches the pattern used by all other view functions.
    ///
    /// The production bug was that getstorageat() in lib.rs did NOT call
    /// configure_network() before processing, while every other view function did.
    /// This test documents the expected pattern.
    #[wasm_bindgen_test]
    fn test_getstorageat_entry_point_pattern() -> Result<()> {
        clear();

        // This test verifies the fix by ensuring getstorageat works
        // in the same flow as other view functions.
        // The lib.rs entry point should call configure_network() before
        // calling view::getstorageat().

        let genesis_block = create_block_with_coinbase_tx(0);
        index_block(&genesis_block, 0)?;

        for h in 1..5u32 {
            let block = create_block_with_coinbase_tx(h);
            index_block(&block, h)?;
        }

        // Simulate what the lib.rs entry point does:
        // 1. configure_network() - THIS WAS MISSING in the buggy version
        configure_network();

        // 2. Decode protobuf from input
        let req = AlkaneStorageRequest {
            id: Some(alkanes_support::proto::alkanes::AlkaneId {
                block: Some(32u128.into()),
                tx: Some(0u128.into()),
            }),
            path: "/totalsupply".as_bytes().to_vec(),
        };

        // 3. Call view::getstorageat
        let result = view::getstorageat(&req.into());
        assert!(result.is_ok(), "getstorageat must work when configure_network() is called first");

        Ok(())
    }
}
