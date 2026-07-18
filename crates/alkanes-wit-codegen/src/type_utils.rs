use alkanes_wit_parser::AlkaneType;
use proc_macro2::TokenStream;
use quote::quote;

/// Convert an AlkaneType to its Rust type tokens.
/// Integer types smaller than u128 are preserved in the generated code.
pub fn type_to_tokens(ty: &AlkaneType) -> TokenStream {
    match ty {
        AlkaneType::U128 => quote! { u128 },
        AlkaneType::U64 => quote! { u64 },
        AlkaneType::U32 => quote! { u32 },
        AlkaneType::U16 => quote! { u16 },
        AlkaneType::U8 => quote! { u8 },
        AlkaneType::Bool => quote! { bool },
        AlkaneType::String => quote! { String },
        AlkaneType::Bytes => quote! { Vec<u8> },
        AlkaneType::AlkaneId => quote! { AlkaneId },
        AlkaneType::List(inner) => {
            let inner_tokens = type_to_tokens(inner);
            quote! { Vec<#inner_tokens> }
        }
        AlkaneType::Option(inner) => {
            let inner_tokens = type_to_tokens(inner);
            quote! { Option<#inner_tokens> }
        }
        AlkaneType::Record(name) | AlkaneType::Enum(name) | AlkaneType::Variant(name) => {
            let ident = quote::format_ident!("{}", name);
            quote! { #ident }
        }
    }
}

/// Convert an AlkaneType to Rust type tokens, widening all integers to u128.
/// This is used for the contract trait and dispatch since the cellpack wire format uses u128.
pub fn type_to_tokens_widened(ty: &AlkaneType) -> TokenStream {
    match ty {
        AlkaneType::U128 | AlkaneType::U64 | AlkaneType::U32 | AlkaneType::U16 => {
            quote! { u128 }
        }
        AlkaneType::U8 => quote! { u128 },
        AlkaneType::List(inner) => {
            let inner_tokens = type_to_tokens_widened(inner);
            quote! { Vec<#inner_tokens> }
        }
        AlkaneType::Option(inner) => {
            let inner_tokens = type_to_tokens_widened(inner);
            quote! { Option<#inner_tokens> }
        }
        // Non-integer types pass through unchanged
        other => type_to_tokens(other),
    }
}

/// Convert an AlkaneType to a string for ABI JSON.
pub fn type_to_abi_string(ty: &AlkaneType) -> String {
    match ty {
        AlkaneType::U128 => "u128".into(),
        AlkaneType::U64 => "u64".into(),
        AlkaneType::U32 => "u32".into(),
        AlkaneType::U16 => "u16".into(),
        AlkaneType::U8 => "u8".into(),
        AlkaneType::Bool => "bool".into(),
        AlkaneType::String => "String".into(),
        AlkaneType::Bytes => "Vec<u8>".into(),
        AlkaneType::AlkaneId => "AlkaneId".into(),
        AlkaneType::List(inner) => format!("Vec<{}>", type_to_abi_string(inner)),
        AlkaneType::Option(inner) => format!("Option<{}>", type_to_abi_string(inner)),
        AlkaneType::Record(name) | AlkaneType::Enum(name) | AlkaneType::Variant(name) => {
            name.clone()
        }
    }
}
