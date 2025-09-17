use crate::message::AlkaneMessageContext;
use metashrew_support::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;

use metashrew_core::environment::MetashrewEnvironment;

pub static TRACES: Lazy<IndexPointer<AlkaneMessageContext<MetashrewEnvironment>>> = Lazy::new(|| IndexPointer::from_keyword("/trace/"));

pub static TRACES_BY_HEIGHT: Lazy<IndexPointer<AlkaneMessageContext<MetashrewEnvironment>>> =
    Lazy::new(|| IndexPointer::from_keyword("/trace/"));
