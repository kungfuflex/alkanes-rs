fn main() {
    alkanes_wit_build::generate(
        "contract.wit",
        "alkanes.toml",
        &format!("{}/generated.rs", std::env::var("OUT_DIR").unwrap()),
    )
    .expect("WIT codegen failed");
}
