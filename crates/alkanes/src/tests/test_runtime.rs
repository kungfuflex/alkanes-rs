use metashrew_support::environment::{EnvironmentInput, RuntimeEnvironment};
use std::collections::HashMap;
use std::sync::{Mutex, Arc};

#[derive(Debug, Clone, Default)]
pub struct TestRuntime {
    store: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    input: Arc<Mutex<Option<EnvironmentInput>>>,
}

impl RuntimeEnvironment for TestRuntime {
    fn get(key: &[u8]) -> Option<Vec<u8>> {
        let runtime = get_runtime();
        let store = runtime.store.lock().unwrap();
        store.get(key).cloned()
    }

    fn flush(data: &[u8]) -> Result<(), ()> {
        let runtime = get_runtime();
        let mut store = runtime.store.lock().unwrap();
        // This is a mock implementation, so we'll just pretend to flush the data
        // by clearing the store and adding the new data.
        store.clear();
        store.insert(b"flushed_data".to_vec(), data.to_vec());
        Ok(())
    }

    fn load_input() -> Result<EnvironmentInput, ()> {
        let runtime = get_runtime();
        let input = runtime.input.lock().unwrap();
        input.clone().ok_or(())
    }

    fn log(message: &str) {
        println!("{}", message);
    }

    fn clear() {
        let runtime = get_runtime();
        let mut store = runtime.store.lock().unwrap();
        store.clear();
    }
}

// This is a thread-safe way to get a singleton instance of the TestRuntime.
pub fn get_runtime() -> Arc<TestRuntime> {
    static RUNTIME: once_cell::sync::Lazy<Arc<TestRuntime>> = once_cell::sync::Lazy::new(|| Arc::new(TestRuntime::default()));
    RUNTIME.clone()
}

pub fn set_input(input: EnvironmentInput) {
    let runtime = get_runtime();
    let mut runtime_input = runtime.input.lock().unwrap();
    *runtime_input = Some(input);
}
