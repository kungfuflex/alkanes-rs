#[cfg(test)]
mod tests {
    use crate::test_helpers::{self as helpers};
    use metashrew::index_pointer::{AtomicPointer, IndexPointer};
    use metashrew_support::index_pointer::KeyValuePointer;
    use std::sync::Arc;
    use wasm_bindgen_test::*;

    // Simple test to verify basic linked list functionality
    #[wasm_bindgen_test]
    fn test_basic_linked_list() {
        helpers::clear();

        // Create a simple IndexPointer directly
        let mut index_ptr = IndexPointer::from_keyword("test");

        // Test append_ll
        index_ptr.append_ll(Arc::new(vec![1, 2, 3]));
        index_ptr.append_ll(Arc::new(vec![4, 5, 6]));

        // Verify length
        assert_eq!(index_ptr.length(), 2);

        // Test map_ll
        let results = index_ptr.map_ll(|item, idx| (idx, item.get()));

        // Verify results
        assert_eq!(results.len(), 2);
        assert_eq!(*results[0].1, vec![1, 2, 3]);
        assert_eq!(*results[1].1, vec![4, 5, 6]);
    }

    #[wasm_bindgen_test]
    fn test_linked_list_operations() {
        helpers::clear();

        // Create a new index pointer directly
        let mut index_ptr = IndexPointer::from_keyword("test_list");

        // Append items to create a linked list
        index_ptr.append_ll(std::sync::Arc::new(vec![1, 2, 3]));
        index_ptr.append_ll(std::sync::Arc::new(vec![4, 5, 6]));
        index_ptr.append_ll(std::sync::Arc::new(vec![7, 8, 9]));
        index_ptr.append_ll(std::sync::Arc::new(vec![10, 11, 12]));
        index_ptr.append_ll(std::sync::Arc::new(vec![13, 14, 15]));

        // Verify the length
        assert_eq!(index_ptr.length(), 5);

        // Test map_ll functionality
        let results = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results
        assert_eq!(results.len(), 5);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[1].0, 1);
        assert_eq!(results[2].0, 2);
        assert_eq!(results[3].0, 3);
        assert_eq!(results[4].0, 4);

        // Verify the values
        assert_eq!(*results[0].1, vec![1, 2, 3]);
        assert_eq!(*results[1].1, vec![4, 5, 6]);
        assert_eq!(*results[2].1, vec![7, 8, 9]);
        assert_eq!(*results[3].1, vec![10, 11, 12]);
        assert_eq!(*results[4].1, vec![13, 14, 15]);

        // Test dropping an index in the middle
        index_ptr.drop_index(2);

        // Test map_ll after dropping an index
        let results_after_drop = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results after dropping
        assert_eq!(results_after_drop.len(), 5);
        assert_eq!(results_after_drop[0].0, 0);
        assert_eq!(results_after_drop[1].0, 1);
        assert_eq!(results_after_drop[2].0, 2); // Index 2 was dropped, but still included in map_ll
        assert_eq!(results_after_drop[3].0, 3);
        assert_eq!(results_after_drop[4].0, 4);

        // Verify the values after dropping
        assert_eq!(*results_after_drop[0].1, vec![1, 2, 3]);
        assert_eq!(*results_after_drop[1].1, vec![4, 5, 6]);
        // Index 2 was dropped, but still included in map_ll with a nullified value
        assert_eq!(*results_after_drop[3].1, vec![10, 11, 12]);
        assert_eq!(*results_after_drop[4].1, vec![13, 14, 15]);
    }

    #[wasm_bindgen_test]
    fn test_linked_list_with_delete_value() {
        helpers::clear();

        // Create a new index pointer directly
        let mut index_ptr = IndexPointer::from_keyword("test_delete");

        // Set up the head key
        let mut head_key = index_ptr.head_key();
        head_key.set_value(0u32);

        // Append items to create a linked list
        index_ptr.append_ll(std::sync::Arc::new(vec![1, 2, 3]));
        index_ptr.append_ll(std::sync::Arc::new(vec![4, 5, 6]));
        index_ptr.append_ll(std::sync::Arc::new(vec![7, 8, 9]));
        index_ptr.append_ll(std::sync::Arc::new(vec![10, 11, 12]));
        index_ptr.append_ll(std::sync::Arc::new(vec![13, 14, 15]));

        // Verify the length
        assert_eq!(index_ptr.length(), 5);

        // Test map_ll functionality
        let results = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results
        assert_eq!(results.len(), 5);

        // Delete a value in the middle using delete_value
        index_ptr.delete_value(2);

        // Test map_ll after deleting a value
        let results_after_delete = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results after deleting
        assert_eq!(results_after_delete.len(), 4); // delete_value properly updates the linked list

        // Verify the values after deleting
        assert_eq!(*results_after_delete[0].1, vec![1, 2, 3]);
        assert_eq!(*results_after_delete[1].1, vec![4, 5, 6]);
        assert_eq!(*results_after_delete[2].1, vec![10, 11, 12]);
        assert_eq!(*results_after_delete[3].1, vec![13, 14, 15]);

        // Delete the head
        index_ptr.delete_value(0);

        // Test map_ll after deleting the head
        let results_after_head_delete = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results after deleting the head - use the actual value from the implementation
        assert_eq!(results_after_head_delete.len(), 3);

        // Verify the values after deleting the head
        assert_eq!(*results_after_head_delete[0].1, vec![4, 5, 6]);
        assert_eq!(*results_after_head_delete[1].1, vec![10, 11, 12]);
        assert_eq!(*results_after_head_delete[2].1, vec![13, 14, 15]);
    }

    #[wasm_bindgen_test]
    fn test_extend_ll_functionality() {
        helpers::clear();

        // Create a new index pointer directly
        let mut index_ptr = IndexPointer::from_keyword("test_extend");

        // Use extend_ll to create a linked list
        let mut item1 = index_ptr.extend_ll();
        item1.set(std::sync::Arc::new(vec![1, 2, 3]));

        let mut item2 = index_ptr.extend_ll();
        item2.set(std::sync::Arc::new(vec![4, 5, 6]));

        let mut item3 = index_ptr.extend_ll();
        item3.set(std::sync::Arc::new(vec![7, 8, 9]));

        // Verify the length
        assert_eq!(index_ptr.length(), 3);

        // Verify the next pointers are set correctly
        assert_eq!(index_ptr.next_key(0).get_value::<u32>(), 1);
        assert_eq!(index_ptr.next_key(1).get_value::<u32>(), 2);

        // Test map_ll functionality
        let results = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results
        assert_eq!(results.len(), 3);
        assert_eq!(*results[0].1, vec![1, 2, 3]);
        assert_eq!(*results[1].1, vec![4, 5, 6]);
        assert_eq!(*results[2].1, vec![7, 8, 9]);

        // Drop the middle item
        index_ptr.drop_index(1);

        // Test map_ll after dropping
        let results_after_drop = index_ptr.map_ll(|item, index| {
            let value = item.get();
            (index, value)
        });

        // Verify results after dropping
        assert_eq!(results_after_drop.len(), 3);
        assert_eq!(*results_after_drop[0].1, vec![1, 2, 3]);
        // Index 1 was dropped, but still included in map_ll with a nullified value
        assert_eq!(*results_after_drop[2].1, vec![7, 8, 9]);
    }

    #[wasm_bindgen_test]
    fn test_map_ll_with_mutations() {
        helpers::clear();

        // Create a new index pointer directly
        let mut index_ptr = IndexPointer::from_keyword("test_mutations");

        // Append items to create a linked list
        index_ptr.append_ll(std::sync::Arc::new(vec![1]));
        index_ptr.append_ll(std::sync::Arc::new(vec![2]));
        index_ptr.append_ll(std::sync::Arc::new(vec![3]));
        index_ptr.append_ll(std::sync::Arc::new(vec![4]));
        index_ptr.append_ll(std::sync::Arc::new(vec![5]));

        // Use map_ll to modify values
        index_ptr.map_ll(|item, _index| {
            let current_value = item.get();
            let value = current_value[0];
            item.set_value(value * 2);
            value
        });

        // Verify the modifications
        let values = index_ptr.map_ll(|item, _index| item.get_value::<u8>());

        assert_eq!(values, vec![2, 4, 6, 8, 10]);

        // Drop an index and verify map_ll still works
        index_ptr.drop_index(2);

        let values_after_drop = index_ptr.map_ll(|item, _index| item.get_value::<u8>());

        // When an index is dropped, it's still included in map_ll but with a value of 0
        assert_eq!(values_after_drop, vec![2, 4, 0, 8, 10]);
    }
}
