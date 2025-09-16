#[cfg(test)]
mod tests {
    mod common;
    use common::clear;
    use anyhow::Result;
    use metashrew_core::{
        index_pointer::IndexPointer,
        println,
        stdio::{stdout, Write},
    };
    use wasm_bindgen_test::wasm_bindgen_test;

    /*
    #[wasm_bindgen_test]
    fn test_response_serialization() -> Result<()> {
        clear();
        Ok(())
    }
    */
}
