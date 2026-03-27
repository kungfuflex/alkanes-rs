use alkanes_wit_parser::AlkaneContractIR;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Generate the `__execute` and `__meta` entry points (equivalent to declare_alkane!).
pub fn generate_entry_points(ir: &AlkaneContractIR) -> TokenStream {
    let contract_name = format_ident!("{}", &ir.name);
    let enum_name = format_ident!("{}Message", &ir.name);

    // Contract struct lives in the parent module
    let contract_path = quote! { super::#contract_name };

    quote! {
        #[no_mangle]
        pub extern "C" fn __execute() -> i32 {
            use alkanes_runtime::runtime::{handle_error, handle_success, prepare_response};
            use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};

            let context = #contract_path::default().context().unwrap();
            let mut inputs = context.inputs.clone();

            if inputs.is_empty() {
                let extended = handle_error("No opcode provided");
                return alkanes_runtime::runtime::response_to_i32(extended);
            }

            let opcode = inputs[0];
            inputs.remove(0);

            let result = match #enum_name::from_opcode(opcode, inputs.clone()) {
                Ok(message) => message.dispatch(&#contract_path::default()),
                Err(_err) => {
                    let instance = #contract_path::default();
                    instance.fallback()
                }
            };

            let extended = match result {
                Ok(res) => handle_success(res),
                Err(err) => {
                    let error_msg = format!("Error: {}", err);
                    let extended = handle_error(&error_msg);
                    return alkanes_runtime::runtime::response_to_i32(extended);
                }
            };

            alkanes_runtime::runtime::response_to_i32(extended)
        }

        #[no_mangle]
        pub extern "C" fn __meta() -> i32 {
            let abi = #enum_name::export_abi();
            __export_bytes(&abi)
        }

        fn __export_bytes(data: &[u8]) -> i32 {
            use metashrew_support::compat::to_arraybuffer_layout;
            let response_bytes = to_arraybuffer_layout(data);
            Box::leak(Box::new(response_bytes)).as_mut_ptr() as usize as i32 + 4
        }
    }
}
