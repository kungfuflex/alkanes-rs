use crate::imports::__log;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
pub use std::fmt::{Error, Write};

pub struct Stdout(());

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        let mut data = to_arraybuffer_layout::<Vec<u8>>(s.to_string().as_bytes().to_vec());
        unsafe {
            __log(to_passback_ptr(&mut data));
        }
        return Ok(());
    }
}

pub fn stdout() -> Stdout {
    Stdout(())
}

#[macro_export]
macro_rules! println {
  ( $( $x:expr ),* ) => {
    {
      writeln!(stdout(), $($x),*).unwrap();
    }
  }
}
