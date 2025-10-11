use bitcoin;
use metashrew_core::environment::{RuntimeEnvironment, MetashrewEnvironment};
use metashrew_support::{compat::export_bytes, index_pointer::{IndexPointer, KeyValuePointer}};
use std::io::Cursor;
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub fn _start() {
    let mut env = MetashrewEnvironment::default();
    let mut input_data = Cursor::new(env.load_input().unwrap().data);
    let height = metashrew_support::utils::consume_sized_int::<u32>(&mut input_data).unwrap();
    let block_bytes = metashrew_support::utils::consume_to_end(&mut input_data).unwrap();
    IndexPointer::from_keyword(format!("/blocks/{}", height).as_str())
        .set(&mut env, Arc::new(block_bytes.clone()));
    let block =
        metashrew_support::utils::consensus_decode::<bitcoin::Block>(&mut Cursor::new(block_bytes))
            .unwrap();
    let mut tracker = IndexPointer::from_keyword("/blocktracker");
    let mut new_tracker = tracker.get(&mut env).as_ref().clone();
    new_tracker.extend((&[block.header.block_hash()[0]]).to_vec());
    tracker.set(&mut env, Arc::new(new_tracker));
    env.flush(&vec![]).unwrap();
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn getblock() -> i32 {
    let mut env = MetashrewEnvironment::default();
    let mut height_bytes = Cursor::new(env.load_input().unwrap().data);
    let height = metashrew_support::utils::consume_sized_int::<u32>(&mut height_bytes).unwrap();
    let key = format!("/blocks/{}", height).into_bytes();
    let block_bytes_arc = IndexPointer::from_keyword(&format!("/blocks/{}", height)).get(&mut env);
    let block_bytes: &Vec<u8> = &*block_bytes_arc;
    export_bytes(block_bytes.clone())
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub fn blocktracker() -> i32 {
    let mut env = MetashrewEnvironment::default();
    export_bytes(
        IndexPointer::from_keyword("/blocktracker")
            .get(&mut env)
            .as_ref()
            .clone(),
    )
}
