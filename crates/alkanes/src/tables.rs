use metashrew_core::index_pointer::IndexPointer;
use metashrew_core::index_pointer::KeyValuePointer;

pub fn traces() -> IndexPointer {
    IndexPointer::from_keyword("/trace/")
}
pub fn traces_by_height() -> IndexPointer {
    IndexPointer::from_keyword("/trace/")
}
