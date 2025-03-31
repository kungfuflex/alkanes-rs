//! WASM exports for Metashrew.
//!
//! This module provides the necessary WASM exports for Metashrew indexer programs.

use anyhow::Result;
use crate::host;
use crate::view;
use crate::indexer::MetashrewIndexer;

/// Define a static variable for the indexer instance
#[macro_export]
macro_rules! define_indexer_instance {
    ($indexer_type:ty) => {
        static mut INDEXER_INSTANCE: Option<$crate::indexer::MetashrewIndexer<$indexer_type>> = None;
    };
}

/// Define the _start function for a Metashrew indexer program
#[macro_export]
macro_rules! define_start_function {
    ($indexer_type:ty) => {
        #[no_mangle]
        pub extern "C" fn _start() {
            unsafe {
                if INDEXER_INSTANCE.is_none() {
                    let indexer = <$indexer_type>::default();
                    INDEXER_INSTANCE = Some($crate::indexer::MetashrewIndexer::new(indexer));
                }

                if let Some(indexer) = &mut INDEXER_INSTANCE {
                    if let Err(e) = indexer.process_block() {
                        $crate::host::log(&format!("Error processing block: {}", e));
                    }
                }
            }
        }
    };
}

/// Define a view function for a Metashrew indexer program
#[macro_export]
macro_rules! define_view_function {
    ($name:ident, $indexer_type:ty) => {
        #[no_mangle]
        pub extern "C" fn $name() -> i32 {
            unsafe {
                if INDEXER_INSTANCE.is_none() {
                    let indexer = <$indexer_type>::default();
                    INDEXER_INSTANCE = Some($crate::indexer::MetashrewIndexer::new(indexer));
                }

                if let Some(indexer) = &INDEXER_INSTANCE {
                    // Load the input data
                    let (_height, input_bytes) = match $crate::host::load_input() {
                        Ok(input) => input,
                        Err(e) => {
                            $crate::host::log(&format!("Error loading input: {}", e));
                            return $crate::view::return_view_result(&[]);
                        }
                    };

                    // Call the view function
                    let result = $crate::view::execute_view_function(
                        indexer.get_indexer(),
                        stringify!($name),
                        &input_bytes,
                    );

                    // Return the result
                    match result {
                        Ok(bytes) => $crate::view::return_view_result(&bytes),
                        Err(e) => {
                            $crate::host::log(&format!("Error executing view function: {}", e));
                            $crate::view::return_view_result(&[])
                        }
                    }
                } else {
                    $crate::view::return_view_result(&[])
                }
            }
        }
    };
}

// The execute_view_function is now defined in view.rs