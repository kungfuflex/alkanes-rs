use alkanes_wit_parser::{AlkaneContractIR, AlkaneReturnType};
use crate::type_utils::type_to_abi_string;
use proc_macro2::TokenStream;
use quote::quote;

/// Generate the ABI export function.
pub fn generate_abi_fn(ir: &AlkaneContractIR) -> TokenStream {
    let contract_name = &ir.name;

    let mut methods_json = String::new();
    let mut first = true;

    for method in &ir.methods {
        if !first {
            methods_json.push_str(", ");
        }
        first = false;

        let mut params_json = String::from("[");
        let mut pfirst = true;
        for param in &method.params {
            if !pfirst {
                params_json.push_str(", ");
            }
            pfirst = false;
            params_json.push_str(&format!(
                "{{ \"type\": \"{}\", \"name\": \"{}\" }}",
                type_to_abi_string(&param.ty),
                param.name
            ));
        }
        params_json.push(']');

        let returns_str = match &method.return_type {
            AlkaneReturnType::CallResponse => "void".to_string(),
            AlkaneReturnType::Typed(ty) => type_to_abi_string(ty),
        };

        methods_json.push_str(&format!(
            "{{ \"name\": \"{}\", \"opcode\": {}, \"params\": {}, \"returns\": \"{}\" }}",
            method.rust_name, method.opcode, params_json, returns_str
        ));
    }

    let abi_string = format!(
        "{{ \"contract\": \"{}\", \"methods\": [{}] }}",
        contract_name, methods_json
    );

    quote! {
        fn __export_abi() -> Vec<u8> {
            #abi_string.as_bytes().to_vec()
        }
    }
}
