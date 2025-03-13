use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Fields, Ident, Lit, LitInt, LitStr, Meta,
    NestedMeta,
};

/// Extracts the opcode attribute from a variant's attributes
fn extract_opcode_attr(attrs: &[Attribute]) -> u128 {
    for attr in attrs {
        if attr.path.is_ident("opcode") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                if let Some(NestedMeta::Lit(Lit::Int(lit_int))) = meta_list.nested.first() {
                    if let Ok(value) = lit_int.base10_parse::<u128>() {
                        return value;
                    }
                }
            }
        }
    }
    panic!("Missing or invalid #[opcode(n)] attribute");
}

/// Extracts the method attribute from a variant's attributes
fn extract_method_attr(attrs: &[Attribute]) -> String {
    for attr in attrs {
        if attr.path.is_ident("method") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                if let Some(NestedMeta::Lit(Lit::Str(lit_str))) = meta_list.nested.first() {
                    return lit_str.value();
                }
            }
        }
    }
    panic!("Missing or invalid #[method(\"name\")] attribute");
}

/// Extracts the param_names attribute from a variant's attributes
fn extract_param_names_attr(
    attrs: &[Attribute],
    expected_count: usize,
    variant_name: &str,
) -> Option<Vec<String>> {
    for attr in attrs {
        if attr.path.is_ident("param_names") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                let param_names = meta_list
                    .nested
                    .iter()
                    .filter_map(|nested_meta| {
                        if let NestedMeta::Lit(Lit::Str(lit_str)) = nested_meta {
                            Some(lit_str.value())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                if !param_names.is_empty() {
                    // Validate that the number of parameter names matches the expected count
                    if param_names.len() != expected_count {
                        panic!(
                            "Number of parameter names ({}) in #[param_names] for variant {} does not match the number of fields ({})",
                            param_names.len(),
                            variant_name,
                            expected_count
                        );
                    }

                    return Some(param_names);
                }
            }
        }
    }
    None
}

/// Derive macro for MessageDispatch trait
#[proc_macro_derive(MessageDispatch, attributes(opcode, method, param_names))]
pub fn derive_message_dispatch(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("MessageDispatch can only be derived for enums"),
    };

    // Generate from_opcode match arms
    let from_opcode_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let opcode = extract_opcode_attr(&variant.attrs);

        let field_count = match &variant.fields {
            Fields::Unnamed(fields) => fields.unnamed.len(),
            Fields::Unit => 0,
            _ => panic!("Named fields are not supported"),
        };

        let param_extractions = match &variant.fields {
            Fields::Unnamed(fields) => {
                let extractions = fields.unnamed.iter().enumerate().map(|(i, _field)| {
                    quote! {
                        inputs.get(#i).cloned().ok_or_else(|| anyhow::anyhow!("Missing parameter"))?
                    }
                });

                quote! { (#(#extractions),*) }
            }
            Fields::Unit => quote! {},
            _ => panic!("Named fields are not supported"),
        };

        quote! {
            #opcode => {
                if inputs.len() < #field_count {
                    return Err(anyhow::anyhow!("Not enough parameters provided"));
                }
                Ok(Self::#variant_name #param_extractions)
            }
        }
    });

    // Generate dispatch match arms
    let dispatch_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let method_str = extract_method_attr(&variant.attrs);
        let method_name = format_ident!("{}", method_str);

        let param_names = match &variant.fields {
            Fields::Unnamed(fields) => {
                let params = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format_ident!("param{}", i));
                quote! { (#(#params),*) }
            }
            Fields::Unit => quote! {},
            _ => panic!("Named fields are not supported"),
        };

        let param_pass_deref = match &variant.fields {
            Fields::Unnamed(fields) => {
                if fields.unnamed.is_empty() {
                    quote! {}
                } else {
                    let params = fields.unnamed.iter().enumerate().map(|(i, _)| {
                        let param = format_ident!("param{}", i);
                        quote! { *#param }
                    });
                    quote! { #(#params),* }
                }
            }
            Fields::Unit => quote! {},
            _ => panic!("Named fields are not supported"),
        };

        quote! {
            Self::#variant_name #param_names => {
                // Call the method directly on the responder
                // This assumes the method exists on the responder
                responder.#method_name(#param_pass_deref)
            }
        }
    });

    // Get the concrete type name by removing "Message" from the enum name
    let name_string = name.to_string();
    let concrete_type_name_string = name_string.trim_end_matches("Message").to_string();
    let concrete_type_name = format_ident!("{}", concrete_type_name_string);

    // Build method JSON entries for ABI
    let mut method_json_entries = String::new();
    let mut first = true;

    for variant in variants.iter() {
        let variant_name = &variant.ident;
        let method_name = extract_method_attr(&variant.attrs);
        let opcode = extract_opcode_attr(&variant.attrs);

        // Determine parameter count based on the variant fields
        let field_count = match &variant.fields {
            Fields::Unnamed(fields) => fields.unnamed.len(),
            Fields::Unit => 0,
            _ => panic!("Named fields are not supported"),
        };

        // Get parameter names if provided, with validation
        let param_names_opt =
            extract_param_names_attr(&variant.attrs, field_count, &variant_name.to_string());

        // Generate parameter JSON
        let mut params_json = String::new();
        if field_count > 0 {
            params_json.push_str("[");
            for i in 0..field_count {
                if i > 0 {
                    params_json.push_str(", ");
                }

                let param_name = if let Some(ref names) = param_names_opt {
                    if i < names.len() {
                        names[i].clone()
                    } else {
                        format!("param{}", i)
                    }
                } else {
                    format!("param{}", i)
                };

                params_json.push_str(&format!(
                    "{{ \"type\": \"u128\", \"name\": \"{}\" }}",
                    param_name
                ));
            }
            params_json.push_str("]");
        } else {
            params_json.push_str("[]");
        }

        // Create the complete method JSON
        let method_json = format!(
            "{{ \"name\": \"{}\", \"opcode\": {}, \"params\": {} }}",
            method_name, opcode, params_json
        );

        if !first {
            method_json_entries.push_str(", ");
        }
        method_json_entries.push_str(&method_json);
        first = false;
    }

    let method_json_str = format!("{}", method_json_entries);

    let expanded = quote! {
        impl alkanes_runtime::message::MessageDispatch<#concrete_type_name> for #name {
            fn from_opcode(opcode: u128, inputs: Vec<u128>) -> Result<Self, anyhow::Error> {
                match opcode {
                    #(#from_opcode_arms)*
                    _ => Err(anyhow::anyhow!("Unknown opcode: {}", opcode)),
                }
            }

            fn dispatch(&self, responder: &#concrete_type_name) -> Result<alkanes_support::response::CallResponse, anyhow::Error> {
                match self {
                    #(#dispatch_arms),*
                }
            }

            fn export_abi() -> Vec<u8> {
                // Generate a JSON representation of the ABI with methods
                let abi_string = format!(
                    "{{ \"contract\": \"{}\", \"methods\": [{}] }}",
                    #concrete_type_name_string,
                    #method_json_str
                );

                abi_string.into_bytes()
            }
        }
    };

    TokenStream::from(expanded)
}
