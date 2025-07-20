pub use crate::wasm::{to_arraybuffer_layout, to_passback_ptr};
use crate::{println, stdio::stdout};
use std::{fmt::Write, panic};

pub fn panic_hook(info: &panic::PanicHookInfo) {
    println!("panic! within WASM: {}", info.to_string());
}
