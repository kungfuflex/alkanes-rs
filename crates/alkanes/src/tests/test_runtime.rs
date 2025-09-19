use metashrew_support::environment::{EnvironmentInput, RuntimeEnvironment};
use std::collections::HashMap;
use std::sync::{Mutex, Arc};

#[derive(Debug, Clone, Default)]
pub struct TestRuntime {
    store: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    input: Arc<Mutex<Option<EnvironmentInput>>>,
    cache: HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>,
    to_flush: HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>>,
}

impl RuntimeEnvironment for TestRuntime {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let store = self.store.lock().unwrap();
        store.get(key).cloned()
    }

    fn flush(&mut self, data: &[u8]) -> Result<(), ()> {
        let mut store = self.store.lock().unwrap();
        // This is a mock implementation, so we'll just pretend to flush the data
        // by clearing the store and adding the new data.
        store.clear();
        store.insert(b"flushed_data".to_vec(), data.to_vec());
        Ok(())
    }

    fn load_input(&self) -> Result<EnvironmentInput, ()> {
        let input = self.input.lock().unwrap();
        input.clone().ok_or(())
    }

    fn log(&self, message: &str) {
        println!("{}", message);
    }

    fn clear(&mut self) {
        let mut store = self.store.lock().unwrap();
        store.clear();
    }
    fn cache(&mut self) -> &mut HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
        &mut self.cache
    }

    fn to_flush(&mut self) -> &mut HashMap<Arc<Vec<u8>>, Arc<Vec<u8>>> {
        &mut self.to_flush
    }
}

// This is a thread-safe way to get a singleton instance of the TestRuntime.
pub fn get_runtime() -> TestRuntime {
    TestRuntime::default()
}

pub fn set_input(runtime: &mut TestRuntime, input: EnvironmentInput) {
    let mut runtime_input = runtime.input.lock().unwrap();
    *runtime_input = Some(input);
}
