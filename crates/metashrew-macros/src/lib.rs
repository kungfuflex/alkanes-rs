//! Procedural macros for Metashrew indexers
//!
//! This crate provides procedural macros that simplify the creation of Metashrew indexers
//! by automatically generating the required boilerplate code.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, Pat, Type};

/// Procedural macro to generate the `_start` function for a Metashrew indexer
///
/// This macro transforms a function with signature `fn main(height: u32, block: &[u8])`
/// into the required `_start` function that parses input and calls the user function.
///
/// # Example
///
/// ```rust,ignore
/// #[metashrew_macros::main]
/// pub fn main(height: u32, block: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
///     // Your indexer logic here
///     Ok(())
/// }
/// ```
///
/// This generates:
///
/// ```rust,ignore
/// #[no_mangle]
/// pub fn _start() {
///     let mut host_input = std::io::Cursor::new(metashrew_core::input());
///     let height = metashrew_support::utils::consume_sized_int::<u32>(&mut host_input)
///         .expect("failed to parse height");
///     let input_vec = metashrew_support::utils::consume_to_end(&mut host_input)
///         .expect("failed to parse bytearray from input after height");
///     main(height, &input_vec).expect("failed to run indexer");
///     metashrew_core::flush();
/// }
/// ```
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract function name
    let fn_name = &input_fn.sig.ident;

    // Validate function signature
    if input_fn.sig.inputs.len() != 2 {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "Function must have exactly 2 parameters: height: u32, block: &[u8]",
        )
        .to_compile_error()
        .into();
    }

    // Validate parameter types
    for (i, arg) in input_fn.sig.inputs.iter().enumerate() {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(_pat_ident) = pat_type.pat.as_ref() {
                match i {
                    0 => {
                        // First parameter should be height: u32
                        if let Type::Path(type_path) = pat_type.ty.as_ref() {
                            if type_path.path.segments.last().unwrap().ident != "u32" {
                                return syn::Error::new_spanned(
                                    &pat_type.ty,
                                    "First parameter must be of type u32",
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                    1 => {
                        // Second parameter should be block: &[u8]
                        if let Type::Reference(type_ref) = pat_type.ty.as_ref() {
                            if let Type::Slice(type_slice) = type_ref.elem.as_ref() {
                                if let Type::Path(type_path) = type_slice.elem.as_ref() {
                                    if type_path.path.segments.last().unwrap().ident != "u8" {
                                        return syn::Error::new_spanned(
                                            &pat_type.ty,
                                            "Second parameter must be of type &[u8]",
                                        )
                                        .to_compile_error()
                                        .into();
                                    }
                                } else {
                                    return syn::Error::new_spanned(
                                        &pat_type.ty,
                                        "Second parameter must be of type &[u8]",
                                    )
                                    .to_compile_error()
                                    .into();
                                }
                            } else {
                                return syn::Error::new_spanned(
                                    &pat_type.ty,
                                    "Second parameter must be of type &[u8]",
                                )
                                .to_compile_error()
                                .into();
                            }
                        } else {
                            return syn::Error::new_spanned(
                                &pat_type.ty,
                                "Second parameter must be of type &[u8]",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Generate the _start function and keep the original function
    let expanded = quote! {
        #input_fn

        #[cfg(all(target_arch = "wasm32", not(test)))]
        #[no_mangle]
        pub fn _start() {
            // CRITICAL: Set cache allocation mode to Indexer (all memory to main LRU cache)
            metashrew_core::set_cache_mode(metashrew_core::CacheAllocationMode::Indexer);
            

            let mut host_input = std::io::Cursor::new(metashrew_core::input());
            let height = metashrew_core::utils::consume_sized_int::<u32>(&mut host_input)
                .expect("failed to parse height");
            let input_vec = metashrew_core::utils::consume_to_end(&mut host_input)
                .expect("failed to parse bytearray from input after height");
            #fn_name(height, &input_vec).expect("failed to run indexer");
            metashrew_core::flush();
        }
    };

    TokenStream::from(expanded)
}

/// Procedural macro to generate view functions for Metashrew indexers
///
/// This macro transforms a function with signature `fn view_name(input: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>`
/// into a WASM-compatible view function that handles input parsing and result export.
///
/// # Example
///
/// ```rust,ignore
/// #[metashrew_macros::view]
/// pub fn protorunesbyaddress(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
///     // Your view logic here
///     Ok(b"result".to_vec())
/// }
/// ```
///
/// This generates:
///
/// ```rust,ignore
/// pub fn __protorunesbyaddress(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
///     // Your original function body
/// }
///
/// #[cfg(not(test))]
/// #[no_mangle]
/// pub fn protorunesbyaddress() -> i32 {
///     let mut host_input = std::io::Cursor::new(metashrew_core::input());
///     let _height = metashrew_support::utils::consume_sized_int::<u32>(&mut host_input)
///         .expect("failed to read height from host input");
///     let result = __protorunesbyaddress(&metashrew_support::utils::consume_to_end(&mut host_input)
///         .expect("failed to read input from host environment")).unwrap();
///     metashrew_support::compat::export_bytes(result)
/// }
/// ```
#[proc_macro_attribute]
pub fn view(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract function name and create the internal function name
    let original_fn_name = &input_fn.sig.ident;
    let internal_fn_name =
        syn::Ident::new(&format!("__{}", original_fn_name), original_fn_name.span());

    // Validate function signature
    if input_fn.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "View function must have exactly 1 parameter: input: &[u8]",
        )
        .to_compile_error()
        .into();
    }

    // Validate parameter type
    if let Some(FnArg::Typed(pat_type)) = input_fn.sig.inputs.first() {
        if let Pat::Ident(_pat_ident) = pat_type.pat.as_ref() {
            // Check that parameter is &[u8]
            if let Type::Reference(type_ref) = pat_type.ty.as_ref() {
                if let Type::Slice(type_slice) = type_ref.elem.as_ref() {
                    if let Type::Path(type_path) = type_slice.elem.as_ref() {
                        if type_path.path.segments.last().unwrap().ident != "u8" {
                            return syn::Error::new_spanned(
                                &pat_type.ty,
                                "Parameter must be of type &[u8]",
                            )
                            .to_compile_error()
                            .into();
                        }
                    } else {
                        return syn::Error::new_spanned(
                            &pat_type.ty,
                            "Parameter must be of type &[u8]",
                        )
                        .to_compile_error()
                        .into();
                    }
                } else {
                    return syn::Error::new_spanned(
                        &pat_type.ty,
                        "Parameter must be of type &[u8]",
                    )
                    .to_compile_error()
                    .into();
                }
            } else {
                return syn::Error::new_spanned(&pat_type.ty, "Parameter must be of type &[u8]")
                    .to_compile_error()
                    .into();
            }
        }
    }

    // Create the internal function (renamed original)
    let mut internal_fn = input_fn.clone();
    internal_fn.sig.ident = internal_fn_name.clone();

    // Extract function visibility and attributes
    let fn_vis = &input_fn.vis;
    let fn_attrs = &input_fn.attrs;

    // Extract the function block
    let fn_block = &input_fn.block;

    // Generate the external view function
    let expanded = quote! {
        // Keep the original function with __ prefix
        #(#fn_attrs)*
        #fn_vis fn #internal_fn_name(input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>>
            #fn_block

        // Generate the WASM export function
        #[cfg(not(test))]
        #[no_mangle]
        pub fn #original_fn_name() -> i32 {
            use metashrew_core;
            // CRITICAL: Set cache allocation mode to View (memory split between height-partitioned and API caches)
            metashrew_core::set_cache_mode(metashrew_core::CacheAllocationMode::View);
            

            let mut host_input = std::io::Cursor::new(metashrew_core::input());
            let height = metashrew_core::utils::consume_sized_int::<u32>(&mut host_input)
                .expect("failed to read height from host input");

            // Set view height for height-partitioned caching
            metashrew_core::set_view_for_height(height);

            let result = #internal_fn_name(&metashrew_core::utils::consume_to_end(&mut host_input)
                .expect("failed to read input from host environment")).unwrap();

            // Clear view height and immediate cache
            metashrew_core::clear_view_cache();

            metashrew_core::export_bytes(result)
        }
    };

    TokenStream::from(expanded)
}
