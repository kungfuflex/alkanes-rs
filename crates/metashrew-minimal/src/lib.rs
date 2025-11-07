use bitcoin;
use metashrew_core::{flush, get, index_pointer::IndexPointer, input};
use metashrew_support::{compat::export_bytes, index_pointer::KeyValuePointer};
use std::io::Cursor;
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub fn _start() {
    let mut input_data = Cursor::new(input());
    let height = metashrew_support::utils::consume_sized_int::<u32>(&mut input_data).unwrap();
    let block_bytes = metashrew_support::utils::consume_to_end(&mut input_data).unwrap();
    IndexPointer::from_keyword(format!("/blocks/{}", height).as_str())
        .set(Arc::new(block_bytes.clone()));
    let block =
        metashrew_support::utils::consensus_decode::<bitcoin::Block>(&mut Cursor::new(block_bytes))
            .unwrap();
    let mut tracker = IndexPointer::from_keyword("/blocktracker");
    let mut new_tracker = tracker.get().as_ref().clone();
    new_tracker.extend((&[block.header.block_hash()[0]]).to_vec());
    tracker.set(Arc::new(new_tracker));
    flush();
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn getblock() -> i32 {
    let mut height_bytes = Cursor::new(input());
    let height = metashrew_support::utils::consume_sized_int::<u32>(&mut height_bytes).unwrap();
    let key = format!("/blocks/{}", height).into_bytes();
    let block_bytes_arc = get(Arc::new(key));
    let block_bytes: &Vec<u8> = &*block_bytes_arc;
    export_bytes(block_bytes.clone())
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub fn blocktracker() -> i32 {
    export_bytes(
        IndexPointer::from_keyword("/blocktracker")
            .get()
            .as_ref()
            .clone(),
    )
}
