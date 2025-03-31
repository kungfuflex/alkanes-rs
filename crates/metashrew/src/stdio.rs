use std::sync::Arc;
use crate::host::__log;
pub use std::fmt::{Error, Write};

// Define our own to_arraybuffer_layout and to_passback_ptr functions
pub fn to_arraybuffer_layout(v: &[u8]) -> Vec<u8> {
    let len = v.len() as u32;
    let mut result = Vec::with_capacity(4 + v.len());
    result.extend_from_slice(&len.to_le_bytes());
    result.extend_from_slice(v);
    result
}

pub fn to_passback_ptr(v: &mut [u8]) -> i32 {
    v.as_ptr() as i32
}

pub struct Stdout(());

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        let data = Arc::new(s.to_string().as_bytes().to_vec());
        log(data.clone());
        return Ok(());
    }
}

pub fn stdout() -> Stdout {
    Stdout(())
}

#[macro_export]
macro_rules! println {
  ( $( $x:tt )* ) => {
    {
      writeln!(stdout(), $($x)*).unwrap();
    }
  }
}

#[macro_export]
macro_rules! print {
  ( $( $x:tt )* ) => {
    {
      write!(stdout(), $($x)*).unwrap();
    }
  }
}

#[allow(unused_unsafe)]
pub fn log(v: Arc<Vec<u8>>) -> () {
    unsafe {
        __log(to_passback_ptr(&mut to_arraybuffer_layout(v.as_ref())));
    }
}
