use alkanes_wit_parser::{AlkaneContractIR, AlkaneTypeDefKind};
use crate::type_utils::type_to_tokens;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate Rust struct/enum definitions for custom types, with CellpackEncode/CellpackDecode impls.
pub fn generate_custom_types(ir: &AlkaneContractIR) -> TokenStream {
    let types: Vec<TokenStream> = ir
        .custom_types
        .iter()
        .map(|typedef| {
            let name = format_ident!("{}", &typedef.name);
            match &typedef.kind {
                AlkaneTypeDefKind::Record(fields) => {
                    let field_defs: Vec<TokenStream> = fields
                        .iter()
                        .map(|f| {
                            let fname = format_ident!("{}", &f.name);
                            let ftype = type_to_tokens(&f.ty);
                            quote! { pub #fname: #ftype }
                        })
                        .collect();

                    let encode_fields: Vec<TokenStream> = fields
                        .iter()
                        .map(|f| {
                            let fname = format_ident!("{}", &f.name);
                            quote! { self.#fname.encode_cellpack(output); }
                        })
                        .collect();

                    let decode_fields: Vec<TokenStream> = fields
                        .iter()
                        .map(|f| {
                            let fname = format_ident!("{}", &f.name);
                            let ftype = type_to_tokens(&f.ty);
                            quote! { #fname: <#ftype as CellpackDecode>::decode_cellpack(input, offset)? }
                        })
                        .collect();

                    quote! {
                        #[derive(Debug, Clone, Default)]
                        pub struct #name {
                            #(#field_defs),*
                        }

                        impl CellpackEncode for #name {
                            fn encode_cellpack(&self, output: &mut Vec<u128>) {
                                #(#encode_fields)*
                            }
                        }

                        impl CellpackDecode for #name {
                            fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
                                Ok(Self {
                                    #(#decode_fields),*
                                })
                            }
                        }
                    }
                }
                AlkaneTypeDefKind::Enum(cases) => {
                    let case_idents: Vec<_> = cases.iter().map(|c| format_ident!("{}", c)).collect();
                    let first_case = &case_idents[0];
                    let case_indices: Vec<_> = (0u128..).take(cases.len()).collect();
                    let case_indices2 = case_indices.clone();

                    quote! {
                        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                        pub enum #name {
                            #(#case_idents),*
                        }

                        impl Default for #name {
                            fn default() -> Self {
                                Self::#first_case
                            }
                        }

                        impl CellpackEncode for #name {
                            fn encode_cellpack(&self, output: &mut Vec<u128>) {
                                let disc: u128 = match self {
                                    #(Self::#case_idents => #case_indices,)*
                                };
                                disc.encode_cellpack(output);
                            }
                        }

                        impl CellpackDecode for #name {
                            fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
                                let disc = u128::decode_cellpack(input, offset)?;
                                match disc {
                                    #(#case_indices2 => Ok(Self::#case_idents),)*
                                    _ => Err(anyhow!("invalid {} discriminant: {}", stringify!(#name), disc)),
                                }
                            }
                        }
                    }
                }
                AlkaneTypeDefKind::Variant(cases) => {
                    let case_defs: Vec<TokenStream> = cases
                        .iter()
                        .map(|c| {
                            let cname = format_ident!("{}", &c.name);
                            if let Some(payload) = &c.payload {
                                let ptype = type_to_tokens(payload);
                                quote! { #cname(#ptype) }
                            } else {
                                quote! { #cname }
                            }
                        })
                        .collect();

                    let case_indices: Vec<u128> = (0u128..).take(cases.len()).collect();

                    let encode_arms: Vec<TokenStream> = cases
                        .iter()
                        .zip(case_indices.iter())
                        .map(|(c, idx)| {
                            let cname = format_ident!("{}", &c.name);
                            if c.payload.is_some() {
                                quote! {
                                    Self::#cname(val) => {
                                        output.push(#idx);
                                        val.encode_cellpack(output);
                                    }
                                }
                            } else {
                                quote! {
                                    Self::#cname => {
                                        output.push(#idx);
                                    }
                                }
                            }
                        })
                        .collect();

                    let decode_arms: Vec<TokenStream> = cases
                        .iter()
                        .zip(case_indices.iter())
                        .map(|(c, idx)| {
                            let cname = format_ident!("{}", &c.name);
                            if let Some(payload) = &c.payload {
                                let ptype = type_to_tokens(payload);
                                quote! {
                                    #idx => Ok(Self::#cname(<#ptype as CellpackDecode>::decode_cellpack(input, offset)?))
                                }
                            } else {
                                quote! {
                                    #idx => Ok(Self::#cname)
                                }
                            }
                        })
                        .collect();

                    quote! {
                        #[derive(Debug, Clone)]
                        pub enum #name {
                            #(#case_defs),*
                        }

                        impl CellpackEncode for #name {
                            fn encode_cellpack(&self, output: &mut Vec<u128>) {
                                match self {
                                    #(#encode_arms)*
                                }
                            }
                        }

                        impl CellpackDecode for #name {
                            fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
                                let disc = u128::decode_cellpack(input, offset)?;
                                match disc {
                                    #(#decode_arms,)*
                                    _ => Err(anyhow!("invalid {} discriminant: {}", stringify!(#name), disc)),
                                }
                            }
                        }
                    }
                }
            }
        })
        .collect();

    quote! { #(#types)* }
}
