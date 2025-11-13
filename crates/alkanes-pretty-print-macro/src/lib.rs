extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(PrettyPrint)]
pub fn pretty_print_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl #name {
            pub fn pretty_print(&self) {
                fn print_value(value: &serde_json::Value, indent: usize) {
                    match value {
                        serde_json::Value::Object(map) => {
                            for (key, val) in map {
                                print_indent(indent);
                                println!("🔑 {}:", key);
                                print_value(val, indent + 1);
                            }
                        }
                        serde_json::Value::Array(arr) => {
                            for (i, val) in arr.iter().enumerate() {
                                print_indent(indent);
                                println!("- {}:", i);
                                print_value(val, indent + 1);
                            }
                        }
                        serde_json::Value::String(s) => {
                            print_indent(indent);
                            println!("📜 {}", s);
                        }
                        serde_json::Value::Number(n) => {
                            print_indent(indent);
                            println!("🔢 {}", n);
                        }
                        serde_json::Value::Bool(b) => {
                            print_indent(indent);
                            println!("✔️ {}", b);
                        }
                        serde_json::Value::Null => {
                            print_indent(indent);
                            println!("🚫 null");
                        }
                    }
                }

                fn print_indent(indent: usize) {
                    for _ in 0..indent {
                        print!("  ");
                    }
                }

                let value = serde_json::to_value(self).unwrap();
                print_value(&value, 0);
            }
        }
    };

    TokenStream::from(expanded)
}
