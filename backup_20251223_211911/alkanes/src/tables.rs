use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;

pub static TRACES: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/trace/"));

pub static TRACES_BY_HEIGHT: Lazy<IndexPointer> =
    Lazy::new(|| IndexPointer::from_keyword("/trace/"));
