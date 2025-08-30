use crate::runtime::MetashrewRuntime;
use crate::tests::stateful_benchmark_test::{create_test_block, InMemoryStore};

fn initialize() -> MetashrewRuntime<InMemoryStore> {
    // Initialize logging
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    // Compile the oom-test WASM
    let wasm_path = std::path::PathBuf::from(
        "../../target/wasm32-unknown-unknown/release/metashrew_oom_test.wasm",
    );
    let manifest_path = std::path::PathBuf::from("../../test-wasms/metashrew-oom-test/Cargo.toml");
    let mut command = std::process::Command::new("cargo");
    command.args([
        "build",
        "--target",
        "wasm32-unknown-unknown",
        "--release",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();
    if !output.status.success() {
        panic!(
            "Failed to compile oom-test WASM: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create storage backend
    let store = InMemoryStore::new();

    // Create runtime
    MetashrewRuntime::load(wasm_path, store, vec![]).unwrap()
}

async fn test_oom_harness(
    runtime: &mut MetashrewRuntime<InMemoryStore>,
    num_entries: u32,
    size_of_each_entry: usize,
) {
    runtime
        .process_block(num_entries, &vec![1; size_of_each_entry])
        .await
        .unwrap();

    // Enable stateful views
    runtime.enable_stateful_views().await.unwrap();

    // Repeatedly call the read_intensive_view to trigger OOM
    // for i in 0..num_entries {
    //     let mut input = num_entries.to_le_bytes().to_vec();
    //     input.extend(i.to_le_bytes().to_vec());
    //     let result = runtime
    //         .view("read_intensive_view".to_string(), &input, 0)
    //         .await;
    //     if (i % (num_entries / 10)) == 0 {
    //         let stats_result = runtime
    //             .view("get_cache_stats_view".to_string(), &vec![], 0)
    //             .await
    //             .unwrap();
    //         log::info!(
    //             "Cache stats at iteration {}: {}",
    //             i,
    //             String::from_utf8_lossy(&stats_result)
    //         );
    //     }
    // }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_large_entries_oom() {
    let mut runtime = initialize();
    for i in 1..505 {
        let size_of_each_entry = 1000 * 1024;
        test_oom_harness(&mut runtime, i, size_of_each_entry).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_large_number_oom() {
    let mut runtime = initialize();
    let num_entries = 268310;
    let size_of_each_entry = 1024;
    test_oom_harness(&mut runtime, num_entries, size_of_each_entry).await;
    for i in 1..505 {
        let size_of_each_entry = 1024;
        test_oom_harness(&mut runtime, i, size_of_each_entry).await;
    }
}
