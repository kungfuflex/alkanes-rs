use alkanes_wit_parser::{AlkaneContractIR, AlkaneType};
use crate::type_utils::type_to_tokens_widened;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the message enum and MessageDispatch implementation.
pub fn generate_dispatch(ir: &AlkaneContractIR) -> TokenStream {
    let contract_name_str = &ir.name;
    let contract_name = quote::format_ident!("{}", contract_name_str);
    let enum_name = format_ident!("{}Message", &ir.name);

    // The contract struct lives in the parent module (super::)
    let contract_path = quote! { super::#contract_name };

    // Generate enum variants
    let variants: Vec<TokenStream> = ir
        .methods
        .iter()
        .map(|method| {
            let variant_name = format_ident!("{}", snake_to_pascal(&method.rust_name));
            if method.params.is_empty() {
                quote! { #variant_name }
            } else {
                let fields: Vec<TokenStream> = method
                    .params
                    .iter()
                    .map(|p| {
                        let pname = format_ident!("{}", &p.name);
                        let ptype = type_to_tokens_widened(&p.ty);
                        quote! { #pname: #ptype }
                    })
                    .collect();
                quote! { #variant_name { #(#fields),* } }
            }
        })
        .collect();

    // Generate from_opcode match arms
    let from_opcode_arms: Vec<TokenStream> = ir
        .methods
        .iter()
        .map(|method| {
            let opcode = method.opcode;
            let variant_name = format_ident!("{}", snake_to_pascal(&method.rust_name));

            if method.params.is_empty() {
                quote! {
                    #opcode => Ok(Self::#variant_name),
                }
            } else {
                let extractions: Vec<TokenStream> = method
                    .params
                    .iter()
                    .map(|p| {
                        let pname = format_ident!("{}", &p.name);
                            generate_extraction(&pname, &p.ty)
                    })
                    .collect();

                let field_names: Vec<_> = method
                    .params
                    .iter()
                    .map(|p| format_ident!("{}", &p.name))
                    .collect();

                quote! {
                    #opcode => {
                        let mut __offset: usize = 0;
                        #(#extractions)*
                        Ok(Self::#variant_name { #(#field_names),* })
                    }
                }
            }
        })
        .collect();

    // Generate dispatch match arms
    // Use fully-qualified trait method calls to avoid ambiguity with AlkaneResponder methods
    let trait_name = format_ident!("{}Interface", &ir.name);
    let dispatch_arms: Vec<TokenStream> = ir
        .methods
        .iter()
        .map(|method| {
            let variant_name = format_ident!("{}", snake_to_pascal(&method.rust_name));
            let method_name = format_ident!("{}", &method.rust_name);

            if method.params.is_empty() {
                quote! {
                    Self::#variant_name => #trait_name::#method_name(responder),
                }
            } else {
                let field_names: Vec<_> = method
                    .params
                    .iter()
                    .map(|p| format_ident!("{}", &p.name))
                    .collect();
                let field_names2 = field_names.clone();

                quote! {
                    Self::#variant_name { #(#field_names),* } => #trait_name::#method_name(responder, #(#field_names2.clone()),*),
                }
            }
        })
        .collect();

    quote! {
        #[allow(dead_code)]
        pub enum #enum_name {
            #(#variants),*
        }

        impl MessageDispatch<#contract_path> for #enum_name {
            fn from_opcode(opcode: u128, __macro_inputs: Vec<u128>) -> Result<Self> {
                match opcode {
                    #(#from_opcode_arms)*
                    _ => Err(anyhow!("unknown opcode: {}", opcode)),
                }
            }

            fn dispatch(&self, responder: &#contract_path) -> Result<CallResponse> {
                match self {
                    #(#dispatch_arms)*
                }
            }

            fn export_abi() -> Vec<u8> {
                __export_abi()
            }
        }
    }
}

fn generate_extraction(
    field_name: &proc_macro2::Ident,
    ty: &AlkaneType,
) -> TokenStream {
    // Use CellpackDecode for all types except String which needs the legacy encoding
    match ty {
        AlkaneType::String => {
            // String uses the legacy null-terminated encoding matching the macro output
            quote! {
                let #field_name = {
                    let mut string_bytes = Vec::new();
                    let mut found_null = false;
                    while __offset < __macro_inputs.len() && !found_null {
                        let value = __macro_inputs[__offset];
                        __offset += 1;
                        let bytes = value.to_le_bytes();
                        for byte in bytes {
                            if byte == 0 {
                                found_null = true;
                                break;
                            }
                            string_bytes.push(byte);
                        }
                    }
                    String::from_utf8(string_bytes)
                        .map_err(|e| anyhow!("invalid UTF-8 string: {}", e))?
                };
            }
        }
        AlkaneType::AlkaneId => {
            quote! {
                let #field_name = {
                    if __offset + 1 >= __macro_inputs.len() {
                        return Err(anyhow!("not enough parameters for AlkaneId"));
                    }
                    let block = __macro_inputs[__offset];
                    let tx = __macro_inputs[__offset + 1];
                    __offset += 2;
                    AlkaneId::new(block, tx)
                };
            }
        }
        AlkaneType::U128 => {
            quote! {
                let #field_name = {
                    if __offset >= __macro_inputs.len() {
                        return Err(anyhow!("missing u128 parameter"));
                    }
                    let v = __macro_inputs[__offset];
                    __offset += 1;
                    v
                };
            }
        }
        AlkaneType::U64 | AlkaneType::U32 | AlkaneType::U16 | AlkaneType::U8 => {
            // All integer types widen to u128 in the cellpack
            quote! {
                let #field_name = {
                    if __offset >= __macro_inputs.len() {
                        return Err(anyhow!("missing parameter"));
                    }
                    let v = __macro_inputs[__offset];
                    __offset += 1;
                    v
                };
            }
        }
        AlkaneType::Bool => {
            quote! {
                let #field_name = {
                    if __offset >= __macro_inputs.len() {
                        return Err(anyhow!("missing bool parameter"));
                    }
                    let v = __macro_inputs[__offset] != 0;
                    __offset += 1;
                    v
                };
            }
        }
        _ => {
            // For complex types, use CellpackDecode
            // Use widened types to match the enum field types (all integers widen to u128)
            let rust_type = crate::type_utils::type_to_tokens_widened(ty);
            quote! {
                let #field_name = <#rust_type as CellpackDecode>::decode_cellpack(&__macro_inputs, &mut __offset)?;
            }
        }
    }
}

/// Convert snake_case to PascalCase.
fn snake_to_pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.extend(chars);
                    s
                }
            }
        })
        .collect()
}
