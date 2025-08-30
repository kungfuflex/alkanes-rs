use phf_codegen::Map;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("view_functions.rs");
    let mut f = File::create(&dest_path).unwrap();

    let view_functions_path = Path::new(&out_dir).join("../../../metashrew-macros/view_functions.txt");
    if !view_functions_path.exists() {
        // If the file doesn't exist, create an empty map
        writeln!(
            f,
            "pub static VIEW_FUNCTIONS: phf::Map<&'static str, fn(&[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>>> = phf::phf_map! {{}};"
        )
        .unwrap();
        return;
    }

    let file = File::open(&view_functions_path).unwrap();
    let reader = BufReader::new(file);

    let mut map = Map::new();
    for line in reader.lines() {
        let line = line.unwrap();
        let internal_fn_name = format!("__{}", line);
        map.entry(
            line,
            &format!("crate::{}", internal_fn_name),
        );
    }

    writeln!(
        f,
        "pub static VIEW_FUNCTIONS: phf::Map<&'static str, fn(&[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>>> = {};",
        map.build()
    )
    .unwrap();
    
    // Clean up the file for the next build
    fs::remove_file(view_functions_path).unwrap();
}