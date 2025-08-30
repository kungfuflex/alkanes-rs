use crate::test_utils::{run_test, TestConfig};

#[tokio::test]
async fn test_println() {
    let config = TestConfig {
        indexer: "metashrew-minimal",
        field: "test_println",
        ..Default::default()
    };
    let res = run_test(&config).await;
    assert_eq!(res.output, "hello world\n");
}