use alkanes_wit_parser::{AlkaneContractIR, AlkaneReturnType, AlkaneType};
use crate::type_utils::type_to_tokens;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate cross-contract call client structs for imported interfaces.
pub fn generate_clients(ir: &AlkaneContractIR) -> TokenStream {
    let clients: Vec<TokenStream> = ir
        .imports
        .iter()
        .map(|import| {
            let client_name = format_ident!("{}", &import.rust_client_name);

            let methods: Vec<TokenStream> = import
                .methods
                .iter()
                .map(|method| {
                    let method_name = format_ident!("{}", &method.rust_name);
                    let opcode = method.opcode;

                    let param_defs: Vec<TokenStream> = method
                        .params
                        .iter()
                        .map(|p| {
                            let pname = format_ident!("{}", &p.name);
                            let ptype = type_to_tokens(&p.ty);
                            quote! { #pname: #ptype }
                        })
                        .collect();

                    // Build the inputs vec
                    let input_encoding: Vec<TokenStream> = method
                        .params
                        .iter()
                        .map(|p| {
                            let pname = format_ident!("{}", &p.name);
                            quote! { #pname.encode_cellpack(&mut __inputs); }
                        })
                        .collect();

                    let call_kind = if method.is_view {
                        quote! { caller.staticcall }
                    } else {
                        quote! { caller.call }
                    };

                    let parcel_arg = if method.is_view {
                        quote! { &AlkaneTransferParcel::default() }
                    } else {
                        quote! { outgoing_alkanes }
                    };

                    let extra_param = if method.is_view {
                        quote! {}
                    } else {
                        quote! { , outgoing_alkanes: &AlkaneTransferParcel }
                    };

                    // Return type handling
                    let return_decode = match &method.return_type {
                        AlkaneReturnType::CallResponse => {
                            quote! { Ok(response) }
                        }
                        AlkaneReturnType::Typed(AlkaneType::String) => {
                            quote! {
                                Ok(String::from_utf8(response.data)
                                    .map_err(|e| anyhow!("invalid UTF-8 in response: {}", e))?)
                            }
                        }
                        AlkaneReturnType::Typed(AlkaneType::U128) => {
                            quote! {
                                if response.data.len() < 16 {
                                    return Err(anyhow!("response too short for u128"));
                                }
                                Ok(u128::from_le_bytes(response.data[0..16].try_into().unwrap()))
                            }
                        }
                        AlkaneReturnType::Typed(AlkaneType::Bool) => {
                            quote! {
                                Ok(!response.data.is_empty() && response.data[0] != 0)
                            }
                        }
                        AlkaneReturnType::Typed(AlkaneType::Bytes) => {
                            quote! { Ok(response.data) }
                        }
                        _ => {
                            quote! { Ok(response) }
                        }
                    };

                    let return_type = match &method.return_type {
                        AlkaneReturnType::CallResponse => quote! { Result<CallResponse> },
                        AlkaneReturnType::Typed(AlkaneType::String) => quote! { Result<String> },
                        AlkaneReturnType::Typed(AlkaneType::U128) => quote! { Result<u128> },
                        AlkaneReturnType::Typed(AlkaneType::Bool) => quote! { Result<bool> },
                        AlkaneReturnType::Typed(AlkaneType::Bytes) => quote! { Result<Vec<u8>> },
                        _ => quote! { Result<CallResponse> },
                    };

                    quote! {
                        pub fn #method_name(
                            &self,
                            caller: &impl AlkaneResponder,
                            #(#param_defs,)*
                            #extra_param
                        ) -> #return_type {
                            let mut __inputs: Vec<u128> = vec![#opcode];
                            #(#input_encoding)*
                            let cellpack = Cellpack {
                                target: self.target.clone(),
                                inputs: __inputs,
                            };
                            let response = #call_kind(
                                &cellpack,
                                #parcel_arg,
                                caller.fuel(),
                            )?;
                            #return_decode
                        }
                    }
                })
                .collect();

            quote! {
                /// Type-safe client for cross-contract calls.
                pub struct #client_name {
                    pub target: AlkaneId,
                }

                impl #client_name {
                    pub fn new(target: AlkaneId) -> Self {
                        Self { target }
                    }

                    #(#methods)*
                }
            }
        })
        .collect();

    quote! { #(#clients)* }
}
