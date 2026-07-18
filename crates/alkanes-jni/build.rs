fn main() {
    // The actual FFI generation is handled by alkanes-ffi
    // This build script can be used for additional JVM-specific setup
    println!("cargo:rerun-if-changed=build.rs");
}
