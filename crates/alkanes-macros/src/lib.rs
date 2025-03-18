use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Fields, FieldsNamed, Ident, Lit, LitInt, Meta,
    NestedMeta, Type, TypePath,
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

/// Extracts the returns attribute from a variant's attributes
fn extract_returns_attr(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path.is_ident("returns") {
            // Just get the raw tokens as a string
            let tokens = attr.tokens.clone().to_string();
            
            // Remove the parentheses and any whitespace
            let type_str = tokens.trim_start_matches('(')
                                .trim_end_matches(')')
                                .trim();
            
            if !type_str.is_empty() {
                return Some(type_str.to_string());
            }
        }
    }
    None
}

/// Convert a variant name to a method name (snake_case)
fn variant_to_method_name(variant_name: &Ident) -> String {
    let name = variant_name.to_string();
    if name.is_empty() {
        return name;
    }
    
    // Convert from CamelCase to snake_case
    let mut result = String::new();
    let mut chars = name.chars().peekable();
    
    // Add the first character (lowercase)
    if let Some(first_char) = chars.next() {
        result.push_str(&first_char.to_lowercase().to_string());
    }
    
    // Process the rest of the characters
    while let Some(c) = chars.next() {
        if c.is_uppercase() {
            // Add underscore before uppercase letters
            result.push('_');
            result.push_str(&c.to_lowercase().to_string());
        } else if c.is_numeric() {
            // Check if the previous character is not a number and not an underscore
            if !result.ends_with('_') && !result.chars().last().unwrap_or(' ').is_numeric() {
                result.push('_');
            }
            result.push(c);
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Check if a type is a String
fn is_string_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "String";
        }
    }
    false
}

/// Get a string representation of a Rust type
fn get_type_string(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                segment.ident.to_string()
            } else {
                "unknown".to_string()
            }
        }
        _ => "unknown".to_string(),
    }
}

/// Derive macro for MessageDispatch trait
#[proc_macro_derive(MessageDispatch, attributes(opcode, returns))]
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

        match &variant.fields {
            Fields::Named(fields_named) => {
                // Handle named fields (struct variants)
                let field_count = fields_named.named.len();
                
                // Create a list of field extractions
                let mut extractions = Vec::new();
                
                // Add the index variable declaration
                extractions.push(quote! {
                    let mut input_index = 0;
                });
                
                // Add extractions for each field
                let mut field_assignments = Vec::new();
                
                for field in fields_named.named.iter() {
                    let field_name = field.ident.as_ref().unwrap();
                    
                    if is_string_type(&field.ty) {
                        // For String types, read null-terminated string from inputs
                        extractions.push(quote! {
                            let #field_name = {
                                // Check if we have at least one input for the string
                                if input_index >= inputs.len() {
                                    return Err(anyhow::anyhow!("Not enough parameters provided for string"));
                                }
                                
                                // Extract the string bytes from the inputs until we find a null terminator
                                let mut string_bytes = Vec::new();
                                let mut found_null = false;
                                
                                while input_index < inputs.len() && !found_null {
                                    let value = inputs[input_index];
                                    input_index += 1;
                                    
                                    let bytes = value.to_le_bytes();
                                    
                                    for byte in bytes {
                                        if byte == 0 {
                                            found_null = true;
                                            break;
                                        }
                                        string_bytes.push(byte);
                                    }
                                    
                                    if found_null {
                                        break;
                                    }
                                }
                                
                                // Convert bytes to string
                                String::from_utf8(string_bytes).map_err(|e| anyhow::anyhow!("Invalid UTF-8 string: {}", e))?
                            };
                        });
                    } else {
                        // For non-String types, just extract the value
                        extractions.push(quote! {
                            let #field_name = {
                                if input_index >= inputs.len() {
                                    return Err(anyhow::anyhow!("Missing parameter"));
                                }
                                let value = inputs[input_index];
                                input_index += 1;
                                value
                            };
                        });
                    }
                    
                    field_assignments.push(quote! { #field_name });
                }
                
                // Create the struct initialization
                let struct_init = quote! {
                    Self::#variant_name {
                        #(#field_assignments),*
                    }
                };
                
                quote! {
                    #opcode => {
                        if inputs.len() < #field_count {
                            return Err(anyhow::anyhow!("Not enough parameters provided"));
                        }
                        
                        #(#extractions)*
                        
                        Ok(#struct_init)
                    }
                }
            },
            Fields::Unnamed(_) => {
                // Error for tuple variants
                panic!("Tuple variants are not supported for MessageDispatch. Use named fields (struct variants) instead for variant {}", variant_name);
            },
            Fields::Unit => {
                // Handle unit variants (no fields)
                quote! {
                    #opcode => {
                        Ok(Self::#variant_name)
                    }
                }
            },
        }
    });

    // Generate dispatch match arms
    let dispatch_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let method_name_str = variant_to_method_name(variant_name);
        let method_name = format_ident!("{}", method_name_str);

        match &variant.fields {
            Fields::Named(fields_named) => {
                // Handle named fields (struct variants)
                let field_names: Vec<_> = fields_named.named.iter()
                    .map(|field| field.ident.as_ref().unwrap())
                    .collect();
                
                let pattern = if !field_names.is_empty() {
                    quote! { { #(#field_names),* } }
                } else {
                    quote! { {} }
                };
                
                let param_pass = if !field_names.is_empty() {
                    quote! { #(#field_names.clone()),* }
                } else {
                    quote! {}
                };

                quote! {
                    Self::#variant_name #pattern => {
                        // Call the method directly on the responder
                        responder.#method_name(#param_pass)
                    }
                }
            },
            Fields::Unnamed(_) => {
                // Error for tuple variants
                panic!("Tuple variants are not supported for MessageDispatch. Use named fields (struct variants) instead for variant {}", variant_name);
            },
            Fields::Unit => {
                // Handle unit variants (no fields)
                quote! {
                    Self::#variant_name => {
                        // Call the method directly on the responder
                        responder.#method_name()
                    }
                }
            },
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
        let method_name = variant_to_method_name(variant_name);
        let opcode = extract_opcode_attr(&variant.attrs);
        let returns_type = extract_returns_attr(&variant.attrs)
            .unwrap_or_else(|| "void".to_string());

        // Determine parameter count, types, and names based on the variant fields
        let (field_count, field_types, param_names) = match &variant.fields {
            Fields::Named(fields_named) => {
                let types = fields_named.named.iter()
                    .map(|field| get_type_string(&field.ty))
                    .collect::<Vec<_>>();
                
                let names = fields_named.named.iter()
                    .map(|field| field.ident.as_ref().unwrap().to_string())
                    .collect::<Vec<_>>();
                
                (fields_named.named.len(), types, names)
            },
            Fields::Unnamed(_) => {
                // Error for tuple variants
                panic!("Tuple variants are not supported for MessageDispatch. Use named fields (struct variants) instead for variant {}", variant_name);
            },
            Fields::Unit => (0, Vec::new(), Vec::new()),
        };

        // Generate parameter JSON
        let mut params_json = String::new();
        if field_count > 0 {
            params_json.push_str("[");
            for i in 0..field_count {
                if i > 0 {
                    params_json.push_str(", ");
                }

                let param_name = &param_names[i];
                let param_type = &field_types[i];

                params_json.push_str(&format!(
                    "{{ \"type\": \"{}\", \"name\": \"{}\" }}",
                    param_type, param_name
                ));
            }
            params_json.push_str("]");
        } else {
            params_json.push_str("[]");
        }

        // Create the complete method JSON
        let method_json = format!(
            "{{ \"name\": \"{}\", \"opcode\": {}, \"params\": {}, \"returns\": \"{}\" }}",
            method_name, opcode, params_json, returns_type
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
