#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        metashrew_core::println!($($arg)*);
    };
}