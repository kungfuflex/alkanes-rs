use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::response::CallResponse;
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_ptr},
    index_pointer::KeyValuePointer,
};
use std::sync::Arc;
use alkanes_support::context::Context;
use std::cell::RefCell;


#[derive(Default)]
pub struct NounsArt {
    pub storage: StoragePointer,
    pub descriptor: StoragePointer,
    pub context: RefCell<Option<Context>>,
}

const BODIES_KEY: &str = "/bodies";
const ACCESSORIES_KEY: &str = "/accessories";
const HEADS_KEY: &str = "/heads";
const GLASSES_KEY: &str = "/glasses";
const PALETTES_KEY: &str = "/palettes";

impl NounsArt {
    fn get_storage_for_key(&self, key: &str) -> StoragePointer {
        StoragePointer::from_keyword(key)
    }

    pub fn store_trait(&mut self, key: &str, data: &[u128]) {
        let mut storage = self.get_storage_for_key(key);
        let data_bytes = Arc::new(data.iter()
            .flat_map(|&x| x.to_be_bytes().to_vec())
            .collect::<Vec<u8>>());
        storage.set(data_bytes);
    }

    fn get_trait(&self, key: &str) -> Vec<u8> {
        let storage = self.get_storage_for_key(key);
        storage.get().as_ref().to_vec()
    }

    fn get_descriptor(&self) -> Vec<u8> {
        self.descriptor.get().as_ref().to_vec()
    }

    pub fn is_initialized(&self) -> bool {
        !self.get_descriptor().is_empty()
    }

    pub fn initialize(&mut self) {
        let descriptor_bytes = Arc::new(self.get_descriptor());
        self.descriptor.set(descriptor_bytes);
    }

    fn context(&self) -> Option<Context> {
        self.context.borrow().clone()
    }

    pub fn set_context(&self, context: Context) {
        *self.context.borrow_mut() = Some(context);
    }
}

impl AlkaneResponder for NounsArt {
    fn execute(&self) -> CallResponse {
        let context = self.context().unwrap();
        let mut response = CallResponse::default();

        if !self.is_initialized() {
            panic!("Descriptor address required for initialization");
        }

        if context.inputs.is_empty() {
            panic!("Operation code required");
        }

        match context.inputs[0] {
            10 => {
                // Get bodies trait
                response.data = self.get_trait(BODIES_KEY);
            }
            11 => {
                // Get accessories trait
                response.data = self.get_trait(ACCESSORIES_KEY);
            }
            12 => {
                // Get heads trait
                response.data = self.get_trait(HEADS_KEY);
            }
            13 => {
                // Get glasses trait
                response.data = self.get_trait(GLASSES_KEY);
            }
            14 => {
                // Get color palettes
                response.data = self.get_trait(PALETTES_KEY);
            }
            20 => {
                // Get descriptor
                response.data = self.get_descriptor();
            }
            _ => panic!("Invalid operation code"),
        }

        response
    }
}

#[no_mangle]
pub extern "C" fn __execute() -> i32 {
    let mut response = to_arraybuffer_layout(&NounsArt::default().run());
    to_ptr(&mut response) + 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use alkanes_support::context::Context;

    #[test]
    fn test_nouns_art_initialization() {
        let mut nouns = NounsArt::default();
        assert!(!nouns.is_initialized());
        
        let descriptor_bytes = Arc::new(vec![1, 2, 3, 4]);
        nouns.descriptor.set(descriptor_bytes);
        nouns.initialize();
        assert!(nouns.is_initialized());
    }

    #[test]
    fn test_trait_storage() {
        let mut nouns = NounsArt::default();
        let test_data = vec![1u128, 2u128, 3u128];
        
        nouns.store_trait(BODIES_KEY, &test_data);
        let retrieved = nouns.get_trait(BODIES_KEY);
        
        let expected: Vec<u8> = test_data.iter()
            .flat_map(|&x| x.to_be_bytes().to_vec())
            .collect();
        assert_eq!(retrieved, expected);
    }

    #[test]
    #[should_panic(expected = "Descriptor address required for initialization")]
    fn test_execute_uninitialized() {
        let nouns = NounsArt::default();
        let context = Context {
            inputs: vec![10],
            ..Default::default()
        };
        nouns.set_context(context);
        nouns.execute();
    }

    #[test]
    #[should_panic(expected = "Operation code required")]
    fn test_execute_no_inputs() {
        let mut nouns = NounsArt::default();
        let descriptor_bytes = Arc::new(vec![1, 2, 3, 4]);
        nouns.descriptor.set(descriptor_bytes);
        nouns.initialize();
        
        let context = Context {
            inputs: vec![],
            ..Default::default()
        };
        nouns.set_context(context);
        nouns.execute();
    }

    #[test]
    fn test_execute_get_traits() {
        let mut nouns = NounsArt::default();
        let descriptor_bytes = Arc::new(vec![1, 2, 3, 4]);
        nouns.descriptor.set(descriptor_bytes);
        nouns.initialize();
        
        // Store some test data
        let test_data = vec![1u128, 2u128];
        nouns.store_trait(BODIES_KEY, &test_data);
        
        // Create test context with operation code 10 (get bodies)
        let context = Context {
            inputs: vec![10],
            ..Default::default()
        };
        nouns.set_context(context);
        
        let response = nouns.execute();
        assert_eq!(response.data, nouns.get_trait(BODIES_KEY));
    }
}