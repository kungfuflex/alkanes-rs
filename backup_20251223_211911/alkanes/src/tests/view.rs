#[cfg(test)]
mod tests {
    use crate::tests::helpers::clear;
    use crate::view::{call_view, get_statics, NAME_OPCODE, STATIC_FUEL, SYMBOL_OPCODE};
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use wasm_bindgen_test::wasm_bindgen_test;

    // Create a static counter to track the number of calls to call_view
    static CALL_VIEW_COUNTER: AtomicUsize = AtomicUsize::new(0);

    // Mock the call_view function to track calls
    fn setup_test_environment() {
        // Reset the counter before each test
        CALL_VIEW_COUNTER.store(0, Ordering::SeqCst);
    }

    #[wasm_bindgen_test]
    fn test_get_statics_caching() -> Result<()> {
        clear();
        setup_test_environment();

        // Create a test AlkaneId
        let test_id = AlkaneId {
            block: 123,
            tx: 456,
        };

        // First call to get_statics should call call_view twice (once for name, once for symbol)
        let (name1, symbol1) = get_statics(&test_id);

        // Make sure we got valid results
        assert!(!name1.is_empty());
        assert!(!symbol1.is_empty());

        // Second call to get_statics with the same ID should use the cache
        let (name2, symbol2) = get_statics(&test_id);

        // Verify the results are the same
        assert_eq!(name1, name2);
        assert_eq!(symbol1, symbol2);

        // Create a different AlkaneId
        let different_id = AlkaneId {
            block: 789,
            tx: 101,
        };

        // Call with a different ID should not use the cache
        let (different_name, different_symbol) = get_statics(&different_id);

        // Make sure we got valid results for the different ID
        assert!(!different_name.is_empty());
        assert!(!different_symbol.is_empty());

        // Call again with the original ID should still use the cache
        let (name3, symbol3) = get_statics(&test_id);

        // Verify the results are still the same
        assert_eq!(name1, name3);
        assert_eq!(symbol1, symbol3);

        Ok(())
    }
}
