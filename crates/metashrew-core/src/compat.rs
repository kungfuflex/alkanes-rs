use crate::stdio::log;
use std::panic;
use std::sync::Arc;
pub fn panic_hook(info: &panic::PanicHookInfo) {
    let _ = log(Arc::new(info.to_string().as_bytes().to_vec()));
}
