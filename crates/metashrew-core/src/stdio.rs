#[cfg(target_arch = "wasm32")]
mod imp {
    pub use std::fmt::{Error, Write};
    use std::sync::Arc;
    use crate::wasm;

    pub struct Stdout(());

    impl Write for Stdout {
        fn write_str(&mut self, s: &str) -> Result<(), Error> {
            let data = Arc::new(s.to_string().as_bytes().to_vec());
            wasm::log(data.clone());
            return Ok(());
        }
    }

    pub fn stdout() -> Stdout {
        Stdout(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::Write;
    pub struct Stdout(std::io::Stdout);
    impl std::fmt::Write for Stdout {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
        }
    }
    pub fn stdout() -> Stdout {
        Stdout(std::io::stdout())
    }
}

pub use imp::{stdout, Stdout};
