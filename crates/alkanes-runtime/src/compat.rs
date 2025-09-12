pub use metashrew_core::wasm::{to_arraybuffer_layout, to_passback_ptr};
use crate::{println, stdio::stdout};
use std::fmt::Write;
use std::panic;

pub fn panic_hook(info: &panic::PanicHookInfo) {
    println!("panic! within WASM: {}", info.to_string());
}
