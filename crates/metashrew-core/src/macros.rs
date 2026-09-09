/// Macro to declare an indexer with view functions
///
/// This macro generates the necessary boilerplate code for a Metashrew indexer,
/// including the _start function and any view functions marked with #[view].
///
/// # Example
///
/// ```rust,ignore
/// declare_indexer! {
///     impl MyIndexer {
///         fn index_block(height: u32, block: bitcoin::Block) {
///             // Implement indexer logic
///         }
///
///         #[view]
///         fn get_balance(request: proto::my_indexer::BalanceRequest) -> Result<proto::my_indexer::BalanceResponse, Error> {
///             // Implement view function
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! declare_indexer {
    (
        impl $indexer_name:ident {
            fn index_block($height_param:ident: u32, $block_param:ident: $block_type:ty) $index_block_body:block

            $(
                #[view]
                fn $view_fn_name:ident($view_param_name:ident: $view_param_type:ty) -> Result<$view_return_type:ty, $error_type:ty $(,)?> $view_fn_body:block
            )*
        }
    ) => {
        use protobuf::Message;
        use metashrew_core::{input, flush};

        /// Entry point for the indexer
        #[no_mangle]
        pub extern "C" fn _start() {
            let data = input();

            // First 4 bytes are the height
            let height = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

            // Rest is the block data
            let block_data = &data[4..];

            // Parse the block
            let block = <$block_type>::consensus_decode(&mut std::io::Cursor::new(block_data))
                .expect("Failed to parse block data");

            // Call the user-defined index_block function
            $indexer_name::index_block(height, block);

            // Flush changes to the database
            flush();
        }

        $(
            /// Generated view function
            #[no_mangle]
            pub extern "C" fn $view_fn_name() -> i32 {
                use metashrew_core::stdio::stdout;

                // Get input data
                let data = input();

                // Parse the request protobuf
                let request = match protobuf::Message::parse_from_bytes::<$view_param_type>(&data) {
                    Ok(req) => req,
                    Err(e) => {
                        let error_msg = format!("Failed to parse request: {}", e);
                        stdout(&error_msg);
                        return -1;
                    }
                };

                // Call the user-defined view function
                match $indexer_name::$view_fn_name(request) {
                    Ok(result) => {
                        // Serialize the result
                        match result.write_to_bytes() {
                            Ok(bytes) => {
                                // Export the result bytes
                                metashrew_core::export_bytes(bytes)
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to serialize response: {}", e);
                                stdout(&error_msg);
                                -1
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("View function error: {}", e);
                        stdout(&error_msg);
                        -1
                    }
                }
            }
        )*

        /// Metadata function that exports information about the indexer
        #[no_mangle]
        pub extern "C" fn __meta() -> i32 {
            use metashrew_support::proto::metashrew::{IndexerMetadata, ViewFunction};

            // Create a metadata structure
            let mut metadata = IndexerMetadata::new();

            // Set compiler information
            metadata.compiler_version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown").to_string();
            metadata.rustc_version = option_env!("RUSTC_VERSION").unwrap_or("unknown").to_string();

            // Add view function information
            $(
                let mut view_fn = ViewFunction::new();
                view_fn.name = stringify!($view_fn_name).to_string();
                view_fn.input_type = stringify!($view_param_type).to_string();
                view_fn.output_type = stringify!($view_return_type).to_string();
                metadata.view_functions.push(view_fn);
            )*

            // Serialize and export
            match metadata.write_to_bytes() {
                Ok(bytes) => metashrew_core::export_bytes(bytes),
                Err(_) => -1,
            }
        }
    };
}
