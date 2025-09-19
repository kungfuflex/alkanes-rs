use crate::message::AlkaneMessageContext;
use metashrew_support::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;

use metashrew_core::environment::RuntimeEnvironment;

pub fn traces<E: RuntimeEnvironment>() -> IndexPointer<E> {
    IndexPointer::from_keyword("/trace/")
}
pub fn traces_by_height<E: RuntimeEnvironment>() -> IndexPointer<E> {
    IndexPointer::from_keyword("/trace/")
}
