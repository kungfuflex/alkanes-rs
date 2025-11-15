use alkanes_macros::mapping_variable;
use alkanes_runtime::storage::StoragePointer;
use alkanes_support::id::AlkaneId;
use metashrew_support::index_pointer::KeyValuePointer;

// Helper function to extract keyword path from StoragePointer
fn get_keyword_path(ptr: &StoragePointer) -> String {
    let bytes = ptr.unwrap();
    String::from_utf8(bytes.as_ref().clone()).unwrap()
}

// Test struct that uses the mapping_variable macro
struct TestContract;

impl TestContract {
    // Test with 1 key (u128)
    mapping_variable!(balances: (u128, u128));

    // Test with 2 keys (u128, AlkaneId)
    mapping_variable!(user_tokens: (u128, AlkaneId, u128));

    // Test with 3 keys (u128, String, AlkaneId)
    mapping_variable!(complex_map: (u128, String, AlkaneId, String));

    // Test with 4 keys (all u128)
    mapping_variable!(quad_map: (u128, u128, u128, u128, u128));

    // Test with AlkaneId key
    mapping_variable!(alkane_map: (AlkaneId, u128));

    // Test with String key
    mapping_variable!(string_map: (String, String));

    // Test with 5 keys (mixed types)
    mapping_variable!(five_keys: (u128, AlkaneId, String, u128, AlkaneId, u128));
}

#[test]
fn test_mapping_variable_single_key() {
    let contract = TestContract;

    // Test with u128 key
    let ptr = contract.balances_pointer(123u128);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/balances/123");
}

#[test]
fn test_mapping_variable_two_keys() {
    let contract = TestContract;

    // Test with u128 and AlkaneId keys
    let alkane_id = AlkaneId::new(100, 200);
    let ptr = contract.user_tokens_pointer(42u128, alkane_id);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/user_tokens/42/100:200");
}

#[test]
fn test_mapping_variable_three_keys() {
    let contract = TestContract;

    // Test with u128, String, and AlkaneId keys
    let alkane_id = AlkaneId::new(50, 75);
    let ptr = contract.complex_map_pointer(10u128, "test".to_string(), alkane_id);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/complex_map/10/test/50:75");
}

#[test]
fn test_mapping_variable_four_keys() {
    let contract = TestContract;

    // Test with 4 u128 keys
    let ptr = contract.quad_map_pointer(1u128, 2u128, 3u128, 4u128);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/quad_map/1/2/3/4");
}

#[test]
fn test_mapping_variable_five_keys() {
    let contract = TestContract;

    // Test with 5 keys (mixed types)
    let alkane_id1 = AlkaneId::new(10, 20);
    let alkane_id2 = AlkaneId::new(30, 40);
    let ptr = contract.five_keys_pointer(1u128, alkane_id1, "hello".to_string(), 2u128, alkane_id2);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/five_keys/1/10:20/hello/2/30:40");
}

#[test]
fn test_mapping_variable_alkane_id_key() {
    let contract = TestContract;

    // Test with AlkaneId as the key
    let alkane_id = AlkaneId::new(999, 888);
    let ptr = contract.alkane_map_pointer(alkane_id);
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/alkane_map/999:888");
}

#[test]
fn test_mapping_variable_string_key() {
    let contract = TestContract;

    // Test with String as the key
    let ptr = contract.string_map_pointer("hello".to_string());
    let keyword = get_keyword_path(&ptr);
    assert_eq!(keyword, "/string_map/hello");
}

#[test]
fn test_mapping_variable_keyword_path_format() {
    let contract = TestContract;

    // Verify that each key gets a "/{key_value}" segment
    let ptr1 = contract.balances_pointer(1u128);
    assert_eq!(get_keyword_path(&ptr1), "/balances/1");

    let ptr2 = contract.user_tokens_pointer(10u128, AlkaneId::new(20, 30));
    assert_eq!(get_keyword_path(&ptr2), "/user_tokens/10/20:30");

    let ptr3 = contract.complex_map_pointer(100u128, "key".to_string(), AlkaneId::new(200, 300));
    assert_eq!(get_keyword_path(&ptr3), "/complex_map/100/key/200:300");

    // Test with multiple u128 keys
    let ptr4 = contract.quad_map_pointer(5u128, 10u128, 15u128, 20u128);
    assert_eq!(get_keyword_path(&ptr4), "/quad_map/5/10/15/20");
}

#[test]
fn test_mapping_variable_variable_key_counts() {
    let contract = TestContract;

    // Test that we can handle different numbers of keys correctly
    // 1 key
    let ptr1 = contract.balances_pointer(1u128);
    assert!(get_keyword_path(&ptr1).starts_with("/balances/"));
    assert_eq!(get_keyword_path(&ptr1).split('/').count(), 3); // /balances/1 = 3 segments (empty, balances, 1)

    // 2 keys
    let ptr2 = contract.user_tokens_pointer(1u128, AlkaneId::new(2, 3));
    assert!(get_keyword_path(&ptr2).starts_with("/user_tokens/"));
    assert_eq!(get_keyword_path(&ptr2).split('/').count(), 4); // /user_tokens/1/2:3 = 4 segments

    // 3 keys
    let ptr3 = contract.complex_map_pointer(1u128, "a".to_string(), AlkaneId::new(2, 3));
    assert!(get_keyword_path(&ptr3).starts_with("/complex_map/"));
    assert_eq!(get_keyword_path(&ptr3).split('/').count(), 5); // /complex_map/1/a/2:3 = 5 segments

    // 4 keys
    let ptr4 = contract.quad_map_pointer(1u128, 2u128, 3u128, 4u128);
    assert!(get_keyword_path(&ptr4).starts_with("/quad_map/"));
    assert_eq!(get_keyword_path(&ptr4).split('/').count(), 6); // /quad_map/1/2/3/4 = 6 segments

    // 5 keys
    let ptr5 = contract.five_keys_pointer(
        1u128,
        AlkaneId::new(2, 3),
        "test".to_string(),
        4u128,
        AlkaneId::new(5, 6),
    );
    assert!(get_keyword_path(&ptr5).starts_with("/five_keys/"));
    assert_eq!(get_keyword_path(&ptr5).split('/').count(), 7); // /five_keys/1/2:3/test/4/5:6 = 7 segments
}

#[test]
fn test_mapping_variable_keyword_path_structure() {
    let contract = TestContract;

    // Verify the structure: /map_name/key1/key2/...
    // Each key should add exactly one segment after the map name

    // Single key
    let path1 = get_keyword_path(&contract.balances_pointer(42u128));
    let parts1: Vec<&str> = path1.split('/').filter(|s| !s.is_empty()).collect();
    assert_eq!(parts1.len(), 2); // balances, 42
    assert_eq!(parts1[0], "balances");
    assert_eq!(parts1[1], "42");

    // Two keys
    let path2 = get_keyword_path(&contract.user_tokens_pointer(10u128, AlkaneId::new(20, 30)));
    let parts2: Vec<&str> = path2.split('/').filter(|s| !s.is_empty()).collect();
    assert_eq!(parts2.len(), 3); // user_tokens, 10, 20:30
    assert_eq!(parts2[0], "user_tokens");
    assert_eq!(parts2[1], "10");
    assert_eq!(parts2[2], "20:30");

    // Three keys
    let path3 = get_keyword_path(&contract.complex_map_pointer(
        100u128,
        "test_key".to_string(),
        AlkaneId::new(200, 300),
    ));
    let parts3: Vec<&str> = path3.split('/').filter(|s| !s.is_empty()).collect();
    assert_eq!(parts3.len(), 4); // complex_map, 100, test_key, 200:300
    assert_eq!(parts3[0], "complex_map");
    assert_eq!(parts3[1], "100");
    assert_eq!(parts3[2], "test_key");
    assert_eq!(parts3[3], "200:300");

    // Four keys
    let path4 = get_keyword_path(&contract.quad_map_pointer(1u128, 2u128, 3u128, 4u128));
    let parts4: Vec<&str> = path4.split('/').filter(|s| !s.is_empty()).collect();
    assert_eq!(parts4.len(), 5); // quad_map, 1, 2, 3, 4
    assert_eq!(parts4[0], "quad_map");
    assert_eq!(parts4[1], "1");
    assert_eq!(parts4[2], "2");
    assert_eq!(parts4[3], "3");
    assert_eq!(parts4[4], "4");
}
