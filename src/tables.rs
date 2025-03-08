use metashrew::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::RwLock;
use protobuf::Message;
use alkanes_support::proto;

pub static TRACES: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/trace/"));

pub static TRACES_BY_HEIGHT: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/trace/"));

// Cache for storing complete BlockTrace structures by height
pub static BLOCK_TRACES_CACHE: Lazy<RwLock<HashMap<u64, Vec<u8>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
