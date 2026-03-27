use alkanes_wit_parser::AlkaneContractIR;
use crate::type_utils::type_to_tokens_widened;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the contract trait that developers implement.
pub fn generate_trait(ir: &AlkaneContractIR) -> TokenStream {
    let trait_name = format_ident!("{}Interface", &ir.name);

    let methods: Vec<TokenStream> = ir
        .methods
        .iter()
        .map(|method| {
            let method_name = format_ident!("{}", &method.rust_name);
            let params: Vec<TokenStream> = method
                .params
                .iter()
                .map(|p| {
                    let pname = format_ident!("{}", &p.name);
                    let ptype = type_to_tokens_widened(&p.ty);
                    quote! { #pname: #ptype }
                })
                .collect();

            // All methods return Result<CallResponse> for consistency with AlkaneResponder
            quote! {
                fn #method_name(&self, #(#params),*) -> Result<CallResponse>;
            }
        })
        .collect();

    quote! {
        /// Trait defining the contract interface. Implement this on your contract struct.
        pub trait #trait_name: AlkaneResponder {
            #(#methods)*
        }
    }
}
