use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta,
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

/// Check if a type is an AlkaneId
fn is_alkane_id_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "AlkaneId";
        }
    }
    false
}

/// Check if a type is a u128
fn is_u128_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "u128";
        }
    }
    false
}

/// Check if a type is a Vec
fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            return segment.ident == "Vec";
        }
    }
    false
}

/// Get the inner type of a Vec
fn get_vec_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type);
                    }
                }
            }
        }
    }
    None
}

/// Generate code to extract a String parameter from __macro_inputs
fn generate_string_extraction(field_name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        let #field_name = {
            // Check if we have at least one input for the string
            if input_index >= __macro_inputs.len() {
                return Err(anyhow::anyhow!("Not enough parameters provided for string"));
            }
            
            // Extract the string bytes from the __macro_inputs until we find a null terminator
            let mut string_bytes = Vec::new();
            let mut found_null = false;
            
            while input_index < __macro_inputs.len() && !found_null {
                let value = __macro_inputs[input_index];
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
    }
}

/// Generate code to extract an AlkaneId parameter from __macro_inputs
fn generate_alkane_id_extraction(field_name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        let #field_name = {
            // AlkaneId consists of two u128 values (block and tx)
            if input_index + 1 >= __macro_inputs.len() {
                return Err(anyhow::anyhow!("Not enough parameters provided for AlkaneId"));
            }
            
            let block = __macro_inputs[input_index];
            input_index += 1;
            
            let tx = __macro_inputs[input_index];
            input_index += 1;
            
            alkanes_support::id::AlkaneId::new(block, tx)
        };
    }
}

/// Generate code to extract a u128 parameter from __macro_inputs
fn generate_u128_extraction(field_name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        let #field_name = {
            if input_index >= __macro_inputs.len() {
                return Err(anyhow::anyhow!("Missing u128 parameter"));
            }
            let value = __macro_inputs[input_index];
            input_index += 1;
            value
        };
    }
}

/// Generate code to extract a single element based on its type
fn generate_element_extraction(ty: &Type, element_name: &Ident) -> proc_macro2::TokenStream {
    if is_string_type(ty) {
        generate_string_extraction(element_name)
    } else if is_alkane_id_type(ty) {
        generate_alkane_id_extraction(element_name)
    } else if is_u128_type(ty) {
        generate_u128_extraction(element_name)
    } else if is_vec_type(ty) {
        // For Vec types, get the inner type and generate Vec extraction
        if let Some(inner_type) = get_vec_inner_type(ty) {
            generate_vec_extraction(element_name, inner_type)
        } else {
            panic!("Failed to get inner type for Vec");
        }
    } else {
        // For other types, panic
        panic!("Unsupported type. Only String, AlkaneId, u128, and Vec are supported.");
    }
}

/// Generate code to extract a Vec parameter from __macro_inputs
fn generate_vec_extraction(field_name: &Ident, inner_type: &Type) -> proc_macro2::TokenStream {
    // Create a temporary element name for the extraction
    let element_name = format_ident!("element");
    
    // Generate the extraction code for a single element of the inner type
    let element_extraction = generate_element_extraction(inner_type, &element_name);

    quote! {
        let #field_name = {
            // First read the length
            if input_index >= __macro_inputs.len() {
                return Err(anyhow::anyhow!("Missing length parameter for Vec"));
            }
            let length = __macro_inputs[input_index] as usize;
            input_index += 1;
            
            // Create a vector to hold the elements
            let mut vec = Vec::with_capacity(length);
            
            // Read each element
            for _ in 0..length {
                #element_extraction
                vec.push(#element_name);
            }
            
            vec
        };
    }
}

/// Get a string representation of a Rust type
fn get_type_string(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                // Check if it's a Vec type
                if segment.ident == "Vec" {
                    // Get the inner type of the Vec
                    if let Some(inner_type) = get_vec_inner_type(ty) {
                        return format!("Vec<{}>", get_type_string(inner_type));
                    }
                }
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
                    
                    // Panic if the field name is "__macro_inputs"
                    if field_name == "__macro_inputs" {
                        panic!("Field name '__macro_inputs' is reserved and cannot be used");
                    }
                    
                    // Use the element extraction helper for all field types
                    extractions.push(generate_element_extraction(&field.ty, field_name));
                    
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
                        if __macro_inputs.len() < #field_count {
                            return Err(anyhow::anyhow!("Not enough parameters provided: expected {} but got {}", #field_count, __macro_inputs.len()));
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
            fn from_opcode(opcode: u128, __macro_inputs: Vec<u128>) -> Result<Self, anyhow::Error> {
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

/// Macro to define storage variable helpers
/// 
/// Usage examples:
/// ```ignore
/// storage_variable!(ticket_price: u128);
/// storage_variable!(token: AlkaneId);
/// storage_variable!(name: String);
/// storage_variable!(root: Vec<u8>);
/// storage_variable!(pool_info: PoolInfo);
/// ```
/// 
/// This generates functions based on the type:
/// 
/// For `u128` type, generates:
/// - `{name}_pointer()` - returns StoragePointer
/// - `{name}()` - gets the value
/// - `set_{name}(value)` - sets the value
/// - `increase_{name}(amount)` - increases the value by amount
/// - `decrease_{name}(amount)` - decreases the value by amount (saturating)
/// 
/// For `AlkaneId` and `String` types, generates:
/// - `{name}_pointer()` - returns StoragePointer
/// - `{name}()` - gets the value (returns Result)
/// - `set_{name}(value)` - sets the value
/// 
/// For `Vec<u8>` type, generates:
/// - `{name}_pointer()` - returns StoragePointer
/// - `{name}()` - gets the value (returns Vec<u8>)
/// - `set_{name}(value: Vec<u8>)` - sets the value
/// 
/// For struct types, generates:
/// - `{name}_pointer()` - returns StoragePointer
/// - `{name}()` - gets the value (returns Result)
/// - `set_{name}(value)` - sets the value
/// 
/// For struct types, it requires the struct to implement:
/// - `from_vec(bytes: &[u8]) -> Result<Self>`
/// - `try_to_vec(&self) -> Vec<u8>`
#[proc_macro]
pub fn storage_variable(input: TokenStream) -> TokenStream {
    // Parse the input: name: type
    let parsed = syn::parse_macro_input!(input as StorageVariableInput);
    
    let name = &parsed.name;
    let name_str = name.to_string();
    let pointer_name = format_ident!("{}_pointer", name);
    let set_name = format_ident!("set_{}", name);
    
    let keyword_path = format!("/{}", name_str);
    
    // Generate code based on type
    let expanded = match &parsed.ty {
        StorageVariableType::U128 => {
            let increase_name = format_ident!("increase_{}", name);
            let decrease_name = format_ident!("decrease_{}", name);
            
            quote! {
                fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                    alkanes_runtime::storage::StoragePointer::from_keyword(#keyword_path)
                }
                
                fn #name(&self) -> u128 {
                    self.#pointer_name().get_value::<u128>()
                }
                
                fn #set_name(&self, value: u128) {
                    self.#pointer_name().set_value::<u128>(value);
                }
                
                fn #increase_name(&self, amount: u128) {
                    let current = self.#name();
                    self.#set_name(current + amount);
                }
                
                fn #decrease_name(&self, amount: u128) {
                    let current = self.#name();
                    self.#set_name(current.saturating_sub(amount));
                }
            }
        }
        StorageVariableType::AlkaneId => {
            quote! {
                fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                    alkanes_runtime::storage::StoragePointer::from_keyword(#keyword_path)
                }
                
                fn #name(&self) -> anyhow::Result<alkanes_support::id::AlkaneId> {
                    use std::io::Read;
                    let ptr = self.#pointer_name().get().as_ref().clone();
                    let mut cursor = std::io::Cursor::<Vec<u8>>::new(ptr);
                    let mut buf = [0u8; 16];
                    cursor.read_exact(&mut buf)?;
                    let block = u128::from_le_bytes(buf);
                    cursor.read_exact(&mut buf)?;
                    let tx = u128::from_le_bytes(buf);
                    Ok(alkanes_support::id::AlkaneId::new(block, tx))
                }
                
                fn #set_name(&self, token_id: alkanes_support::id::AlkaneId) {
                    let mut ptr = self.#pointer_name();
                    ptr.set(std::sync::Arc::new(token_id.into()));
                }
            }
        }
        StorageVariableType::String => {
            quote! {
                fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                    alkanes_runtime::storage::StoragePointer::from_keyword(#keyword_path)
                }
                
                fn #name(&self) -> anyhow::Result<String> {
                    let bytes = self.#pointer_name().get().as_ref().clone();
                    if bytes.is_empty() {
                        return Ok(String::new());
                    }
                    
                    if bytes.len() < 4 {
                        return Err(anyhow::anyhow!("Invalid bytes length for String"));
                    }
                    
                    let name_length = u32::from_le_bytes(bytes[0..4].try_into()?) as usize;
                    if bytes.len() < 4 + name_length {
                        return Err(anyhow::anyhow!("Invalid bytes length for String content"));
                    }
                    
                    Ok(String::from_utf8(bytes[4..4 + name_length].to_vec())?)
                }
                
                fn #set_name(&self, value: String) {
                    let mut bytes = Vec::new();
                    let name_bytes = value.as_bytes();
                    bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(name_bytes);
                    
                    let mut ptr = self.#pointer_name();
                    ptr.set(std::sync::Arc::new(bytes));
                }
            }
        }
        StorageVariableType::VecU8 => {
            quote! {
                fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                    alkanes_runtime::storage::StoragePointer::from_keyword(#keyword_path)
                }
                
                fn #name(&self) -> Vec<u8> {
                    self.#pointer_name().get().as_ref().clone()
                }
                
                fn #set_name(&self, v: Vec<u8>) {
                    self.#pointer_name().set(std::sync::Arc::new(v));
                }
            }
        }
        StorageVariableType::Struct(struct_name) => {
            quote! {
                fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                    alkanes_runtime::storage::StoragePointer::from_keyword(#keyword_path)
                }
                
                fn #name(&self) -> anyhow::Result<#struct_name> {
                    let bytes = self.#pointer_name().get().as_ref().clone();
                    #struct_name::from_vec(&bytes)
                }
                
                fn #set_name(&self, value: #struct_name) {
                    let mut ptr = self.#pointer_name();
                    ptr.set(std::sync::Arc::new(value.try_to_vec()));
                }
            }
        }
    };
    
    TokenStream::from(expanded)
}

/// Macro to define mapping storage variable helpers
/// 
/// Usage examples:
/// ```ignore
/// mapping_variable!(balances: (AlkaneId, u128));
/// mapping_variable!(names: (u128, String));
/// mapping_variable!(triple_index: (u128, AlkaneId, u128, u128));
/// mapping_variable!(data: (String, Vec<u8>));
/// ```
/// 
/// This generates functions for key-value mappings:
/// - `{map_name}_pointer(&self, key_0: KeyType0, key_1: KeyType1, ...) -> StoragePointer`
/// - `{map_name}(&self, key_0: KeyType0, key_1: KeyType1, ...) -> ValueType` (or Result<ValueType>)
/// - `set_{map_name}(&self, key_0: KeyType0, key_1: KeyType1, ..., value: ValueType)`
/// 
/// Storage path format: "/{map_name}/{key_0}/{key_1}/..." with `Vec<u8>` segments appended as raw bytes after a slash
/// 
/// Supported key types: u128, AlkaneId, String, Vec<u8>
/// Supported value types: u128, AlkaneId, String, Vec<u8>, structs (with from_vec/try_to_vec)
#[proc_macro]
pub fn mapping_variable(input: TokenStream) -> TokenStream {
    // Parse the input: map_name: (KeyType0, KeyType1, ..., ValueType)
    let parsed = syn::parse_macro_input!(input as MappingVariableInput);
    
    let map_name = &parsed.map_name;
    let map_name_str = map_name.to_string();
    let pointer_name = format_ident!("{}_pointer", map_name);
    let set_name = format_ident!("set_{}", map_name);
    
    // Generate code based on key type and value type
    let expanded = generate_mapping_functions(
        &pointer_name,
        map_name,
        &map_name_str,
        &set_name,
        &parsed.key_types,
        &parsed.value_type,
    );
    
    TokenStream::from(expanded)
}

fn textual_segment_expr(key_ident: &Ident, key_type: &MappingKeyType) -> proc_macro2::TokenStream {
    match key_type {
        MappingKeyType::U128 | MappingKeyType::String => quote! { format!("/{}", #key_ident) },
        MappingKeyType::AlkaneId => {
            quote! { format!("/{block}:{tx}", block = #key_ident.block, tx = #key_ident.tx) }
        }
        MappingKeyType::VecU8 => panic!("Vec<u8> keys are not textual segments"),
    }
}

fn generate_pointer_body(
    map_name_str: &str,
    key_types: &[MappingKeyType],
    key_idents: &[Ident],
) -> proc_macro2::TokenStream {
    let base_init = quote! {
        let mut keyword_path = format!("/{}", #map_name_str);
    };

    let first_vec_index = key_types
        .iter()
        .position(|kt| matches!(kt, MappingKeyType::VecU8));

    if let Some(first_vec_index) = first_vec_index {
        let before_vec_statements =
            key_types
                .iter()
                .enumerate()
                .take(first_vec_index)
                .map(|(idx, key_type)| {
                    let key_ident = &key_idents[idx];
                    let segment_expr = textual_segment_expr(key_ident, key_type);
                    quote! {
                        keyword_path.push_str(&#segment_expr);
                    }
                });

        let mut after_vec_statements = Vec::new();

        for (idx, key_type) in key_types.iter().enumerate().skip(first_vec_index) {
            let key_ident = &key_idents[idx];
            match key_type {
                MappingKeyType::VecU8 => {
                    if idx > first_vec_index {
                        let delimiter_ident = format_ident!("__keyword_delimiter_{}", idx);
                        after_vec_statements.push(quote! {
                            let #delimiter_ident = vec![b'/'];
                            pointer = pointer.select(&#delimiter_ident);
                        });
                    }
                    after_vec_statements.push(quote! {
                        pointer = pointer.select(&#key_ident);
                    });
                }
                _ => {
                    let segment_ident = format_ident!("__keyword_segment_{}", idx);
                    let segment_bytes_ident = format_ident!("__keyword_segment_bytes_{}", idx);
                    let segment_expr = textual_segment_expr(key_ident, key_type);
                    after_vec_statements.push(quote! {
                        let #segment_ident = #segment_expr;
                        let #segment_bytes_ident = #segment_ident.into_bytes();
                        pointer = pointer.select(&#segment_bytes_ident);
                    });
                }
            }
        }

        quote! {
            #base_init
            #(#before_vec_statements)*
            keyword_path.push('/');
            let mut pointer = alkanes_runtime::storage::StoragePointer::from_keyword(&keyword_path);
            #(#after_vec_statements)*
            pointer
        }
    } else {
        let segment_statements = key_types.iter().enumerate().map(|(idx, key_type)| {
            let key_ident = &key_idents[idx];
            let segment_expr = textual_segment_expr(key_ident, key_type);
            quote! {
                keyword_path.push_str(&#segment_expr);
            }
        });

        quote! {
            #base_init
            #(#segment_statements)*
            alkanes_runtime::storage::StoragePointer::from_keyword(&keyword_path)
        }
    }
}

fn key_type_to_tokens(key_type: &MappingKeyType) -> proc_macro2::TokenStream {
    match key_type {
        MappingKeyType::U128 => quote! { u128 },
        MappingKeyType::AlkaneId => quote! { alkanes_support::id::AlkaneId },
        MappingKeyType::String => quote! { String },
        MappingKeyType::VecU8 => quote! { Vec<u8> },
    }
}

#[allow(dead_code)]
fn value_type_to_tokens(value_type: &MappingValueType) -> proc_macro2::TokenStream {
    match value_type {
        MappingValueType::U128 => quote! { u128 },
        MappingValueType::AlkaneId => quote! { alkanes_support::id::AlkaneId },
        MappingValueType::String => quote! { String },
        MappingValueType::VecU8 => quote! { Vec<u8> },
        MappingValueType::Struct(struct_name) => quote! { #struct_name },
    }
}

fn generate_mapping_functions(
    pointer_name: &Ident,
    map_name: &Ident,
    map_name_str: &str,
    set_name: &Ident,
    key_types: &[MappingKeyType],
    value_type: &MappingValueType,
) -> proc_macro2::TokenStream {
    assert!(
        !key_types.is_empty(),
        "mapping_variable! requires at least one key type"
    );

    let key_idents: Vec<Ident> = key_types
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("key_{}", idx))
        .collect();

    let key_params: Vec<proc_macro2::TokenStream> = key_types
        .iter()
        .enumerate()
        .map(|(idx, key_type)| {
            let ident = &key_idents[idx];
            let ty_tokens = key_type_to_tokens(key_type);
            quote! { #ident: #ty_tokens }
        })
        .collect();

    let pointer_body = generate_pointer_body(map_name_str, key_types, &key_idents);
    let pointer_params = key_params.clone();

    let pointer_fn = if pointer_params.is_empty() {
        quote! {
            fn #pointer_name(&self) -> alkanes_runtime::storage::StoragePointer {
                #pointer_body
            }
        }
    } else {
        quote! {
            fn #pointer_name(&self, #(#pointer_params),*) -> alkanes_runtime::storage::StoragePointer {
                #pointer_body
            }
        }
    };

    // Generate get and set functions based on value type
    let (get_fn, set_fn) = match value_type {
        MappingValueType::U128 => {
            let get_params = key_params.clone();
            let set_params = key_params.clone();
            let pointer_args_get = key_idents.clone();
            let pointer_args_set = key_idents.clone();

            let get_quote = quote! {
                fn #map_name(&self, #(#get_params),*) -> u128 {
                    self.#pointer_name(#(#pointer_args_get),*).get_value::<u128>()
                }
            };
            let set_quote = quote! {
                fn #set_name(&self, #(#set_params),*, value: u128) {
                    self.#pointer_name(#(#pointer_args_set),*).set_value::<u128>(value);
                }
            };
            (get_quote, set_quote)
        }
        MappingValueType::AlkaneId => {
            let get_params = key_params.clone();
            let set_params = key_params.clone();
            let pointer_args_get = key_idents.clone();
            let pointer_args_set = key_idents.clone();

            let get_quote = quote! {
                fn #map_name(&self, #(#get_params),*) -> anyhow::Result<alkanes_support::id::AlkaneId> {
                    use std::io::Read;
                    let ptr = self.#pointer_name(#(#pointer_args_get),*).get().as_ref().clone();
                    let mut cursor = std::io::Cursor::<Vec<u8>>::new(ptr);
                    let mut buf = [0u8; 16];
                    cursor.read_exact(&mut buf)?;
                    let block = u128::from_le_bytes(buf);
                    cursor.read_exact(&mut buf)?;
                    let tx = u128::from_le_bytes(buf);
                    Ok(alkanes_support::id::AlkaneId::new(block, tx))
                }
            };
            let set_quote = quote! {
                fn #set_name(&self, #(#set_params),*, value: alkanes_support::id::AlkaneId) {
                    let mut ptr = self.#pointer_name(#(#pointer_args_set),*);
                    ptr.set(std::sync::Arc::new(value.into()));
                }
            };
            (get_quote, set_quote)
        }
        MappingValueType::String => {
            let get_params = key_params.clone();
            let set_params = key_params.clone();
            let pointer_args_get = key_idents.clone();
            let pointer_args_set = key_idents.clone();

            let get_quote = quote! {
                fn #map_name(&self, #(#get_params),*) -> anyhow::Result<String> {
                    let bytes = self.#pointer_name(#(#pointer_args_get),*).get().as_ref().clone();
                    if bytes.is_empty() {
                        return Ok(String::new());
                    }
                    
                    if bytes.len() < 4 {
                        return Err(anyhow::anyhow!("Invalid bytes length for String"));
                    }
                    
                    let name_length = u32::from_le_bytes(bytes[0..4].try_into()?) as usize;
                    if bytes.len() < 4 + name_length {
                        return Err(anyhow::anyhow!("Invalid bytes length for String content"));
                    }
                    
                    Ok(String::from_utf8(bytes[4..4 + name_length].to_vec())?)
                }
            };
            let set_quote = quote! {
                fn #set_name(&self, #(#set_params),*, value: String) {
                    let mut bytes = Vec::new();
                    let name_bytes = value.as_bytes();
                    bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(name_bytes);
                    
                    let mut ptr = self.#pointer_name(#(#pointer_args_set),*);
                    ptr.set(std::sync::Arc::new(bytes));
                }
            };
            (get_quote, set_quote)
        }
        MappingValueType::VecU8 => {
            let get_params = key_params.clone();
            let set_params = key_params.clone();
            let pointer_args_get = key_idents.clone();
            let pointer_args_set = key_idents.clone();

            let get_quote = quote! {
                fn #map_name(&self, #(#get_params),*) -> Vec<u8> {
                    self.#pointer_name(#(#pointer_args_get),*).get().as_ref().clone()
                }
            };
            let set_quote = quote! {
                fn #set_name(&self, #(#set_params),*, v: Vec<u8>) {
                    self.#pointer_name(#(#pointer_args_set),*).set(std::sync::Arc::new(v));
                }
            };
            (get_quote, set_quote)
        }
        MappingValueType::Struct(struct_name) => {
            let get_params = key_params.clone();
            let set_params = key_params.clone();
            let pointer_args_get = key_idents.clone();
            let pointer_args_set = key_idents.clone();

            let get_quote = quote! {
                fn #map_name(&self, #(#get_params),*) -> anyhow::Result<#struct_name> {
                    let bytes = self.#pointer_name(#(#pointer_args_get),*).get().as_ref().clone();
                    #struct_name::from_vec(&bytes)
                }
            };
            let set_quote = quote! {
                fn #set_name(&self, #(#set_params),*, value: #struct_name) {
                    let mut ptr = self.#pointer_name(#(#pointer_args_set),*);
                    ptr.set(std::sync::Arc::new(value.try_to_vec()));
                }
            };
            (get_quote, set_quote)
        }
    };
    
    quote! {
        #pointer_fn
        #get_fn
        #set_fn
    }
}

enum ParsedType {
    U128,
    AlkaneId,
    String,
    VecU8,
    Struct(Ident),
}

fn parse_type(ty_type: &Type) -> syn::Result<ParsedType> {
    match ty_type {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let seg_name = segment.ident.to_string();
                
                // Check for Vec<u8>
                if seg_name == "Vec" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                            if let Type::Path(inner_path) = inner_type {
                                if let Some(inner_seg) = inner_path.path.segments.last() {
                                    if inner_seg.ident == "u8" {
                                        return Ok(ParsedType::VecU8);
                                    }
                                }
                            }
                        }
                    }
                    return Err(syn::Error::new_spanned(ty_type, "Only Vec<u8> is supported for Vec types"));
                }
                
                // Check for simple types
                Ok(match seg_name.as_str() {
                    "u128" => ParsedType::U128,
                    "AlkaneId" => ParsedType::AlkaneId,
                    "String" => ParsedType::String,
                    _ => ParsedType::Struct(segment.ident.clone()),
                })
            } else {
                Err(syn::Error::new_spanned(ty_type, "Invalid type"))
            }
        }
        _ => Err(syn::Error::new_spanned(ty_type, "Expected type identifier or generic type")),
    }
}

enum StorageVariableType {
    U128,
    AlkaneId,
    String,
    VecU8,
    Struct(Ident),
}

struct StorageVariableInput {
    name: Ident,
    ty: StorageVariableType,
}

impl syn::parse::Parse for StorageVariableInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<syn::Token![:]>()?;
        
        let ty_type: Type = input.parse()?;
        let parsed = parse_type(&ty_type)?;
        
        let ty = match parsed {
            ParsedType::U128 => StorageVariableType::U128,
            ParsedType::AlkaneId => StorageVariableType::AlkaneId,
            ParsedType::String => StorageVariableType::String,
            ParsedType::VecU8 => StorageVariableType::VecU8,
            ParsedType::Struct(ident) => StorageVariableType::Struct(ident),
        };
        
        Ok(StorageVariableInput { name, ty })
    }
}

#[derive(Debug, PartialEq)]
enum MappingKeyType {
    U128,
    AlkaneId,
    String,
    VecU8,
}

enum MappingValueType {
    U128,
    AlkaneId,
    String,
    VecU8,
    Struct(Ident),
}

struct MappingVariableInput {
    map_name: Ident,
    key_types: Vec<MappingKeyType>,
    value_type: MappingValueType,
}

impl syn::parse::Parse for MappingVariableInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let map_name: Ident = input.parse()?;
        input.parse::<syn::Token![:]>()?;
        
        let content;
        syn::parenthesized!(content in input);
        let types: syn::punctuated::Punctuated<Type, syn::Token![,]> =
            content.parse_terminated(Type::parse)?;
        
        if types.len() < 2 {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "mapping_variable! requires at least one key type and one value type",
            ));
        }
        
        let mut type_vec: Vec<Type> = types.into_iter().collect();
        let value_type_ty = type_vec
            .pop()
            .expect("type_vec has at least one element after len check");
        let value_parsed = parse_type(&value_type_ty)?;
        let value_type = match value_parsed {
            ParsedType::U128 => MappingValueType::U128,
            ParsedType::AlkaneId => MappingValueType::AlkaneId,
            ParsedType::String => MappingValueType::String,
            ParsedType::VecU8 => MappingValueType::VecU8,
            ParsedType::Struct(ident) => MappingValueType::Struct(ident),
        };
        
        let mut key_types = Vec::with_capacity(type_vec.len());
        for key_type_ty in type_vec {
            let key_parsed = parse_type(&key_type_ty)?;
            let key_type = match key_parsed {
                ParsedType::U128 => MappingKeyType::U128,
                ParsedType::AlkaneId => MappingKeyType::AlkaneId,
                ParsedType::String => MappingKeyType::String,
                ParsedType::VecU8 => MappingKeyType::VecU8,
                ParsedType::Struct(_) => {
                    return Err(syn::Error::new_spanned(
                        &key_type_ty,
                        "Unsupported key type. Supported: u128, AlkaneId, String, Vec<u8>",
                    ))
                }
            };
            key_types.push(key_type);
        }
        
        Ok(MappingVariableInput {
            map_name,
            key_types,
            value_type,
        })
    }
}
