fn main() {
    // Only generate scaffolding if the UDL file exists
    let udl_file = "src/alkanes.udl";
    if std::path::Path::new(udl_file).exists() {
        uniffi::generate_scaffolding(udl_file).unwrap();
    } else {
        println!("cargo:warning=UDL file not found, skipping scaffolding generation");
    }
}
