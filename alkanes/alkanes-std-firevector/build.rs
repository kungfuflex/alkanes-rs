fn main() {
    // The VM core is deliberately buildable and testable without the alkanes
    // runtime, so WIT codegen only runs when the `alkane` surface is requested.
    if std::env::var("CARGO_FEATURE_ALKANE").is_err() {
        return;
    }
    alkanes_wit_build::generate(
        "contract.wit",
        "alkanes.toml",
        &format!("{}/generated.rs", std::env::var("OUT_DIR").unwrap()),
    )
    .expect("WIT codegen failed");
}
