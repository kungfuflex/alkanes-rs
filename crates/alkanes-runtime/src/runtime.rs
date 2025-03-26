#[allow(unused_imports)]
use crate::imports::{
    __balance, __call, __delegatecall, __fuel, __height, __load_block, __load_context,
    __load_storage, __load_transaction, __log, __request_block, __request_context,
    __request_storage, __request_transaction, __returndatacopy, __sequence, __staticcall,
    abort, /*, __load_output, __request_output */
};
#[allow(unused_imports)]
use crate::{
    println,
    stdio::{stdout, Write},
};
use anyhow::{anyhow, Result};
#[allow(unused_imports)]
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr};
use std::io::Cursor;

#[cfg(feature = "panic-hook")]
use crate::compat::panic_hook;

#[allow(unused_imports)]
use alkanes_support::{
    cellpack::Cellpack,
    context::Context,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::{CallResponse, ExtendedCallResponse},
    storage::StorageMap,
};
#[cfg(feature = "panic-hook")]
use std::panic;

fn _abort() {
    unsafe {
        abort(0, 0, 0, 0);
    }
}

static mut _CACHE: Option<StorageMap> = None;

pub fn initialize_cache() {
    unsafe {
        if _CACHE.is_none() {
            _CACHE = Some(StorageMap::default());
        }
    }
}

#[allow(static_mut_refs)]
pub fn get_cache() -> StorageMap {
    unsafe {
        initialize_cache();
        _CACHE.as_ref().unwrap().clone()
    }
}

#[allow(static_mut_refs)]
pub fn handle_success(response: CallResponse) -> ExtendedCallResponse {
    let mut extended: ExtendedCallResponse = response.into();
    initialize_cache();
    extended.storage = get_cache();
    extended
}

pub fn handle_error(error: &str) -> ExtendedCallResponse {
    let mut response = CallResponse::default();
    let mut data: Vec<u8> = vec![0x08, 0xc3, 0x79, 0xa0];
    data.extend(error.as_bytes());
    response.data = data;
    _abort();
    response.into()
}

pub fn prepare_response(response: ExtendedCallResponse) -> Vec<u8> {
    response.serialize()
}

pub fn response_to_i32(response: ExtendedCallResponse) -> i32 {
    let serialized = prepare_response(response);
    let response_bytes = to_arraybuffer_layout(&serialized);
    Box::leak(Box::new(response_bytes)).as_mut_ptr() as usize as i32 + 4
}

pub trait Extcall {
    fn __call(cellpack: i32, outgoing_alkanes: i32, checkpoint: i32, fuel: u64) -> i32;
    #[allow(static_mut_refs)]
    fn call(
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        let mut cellpack_buffer = to_arraybuffer_layout::<&[u8]>(&cellpack.serialize());
        let mut outgoing_alkanes_buffer: Vec<u8> =
            to_arraybuffer_layout::<&[u8]>(&outgoing_alkanes.serialize());
        let mut storage_map_buffer =
            to_arraybuffer_layout::<&[u8]>(&unsafe { _CACHE.as_ref().unwrap().serialize() });
        let _call_result = Self::__call(
            to_passback_ptr(&mut cellpack_buffer),
            to_passback_ptr(&mut outgoing_alkanes_buffer),
            to_passback_ptr(&mut storage_map_buffer),
            fuel,
        );
        if _call_result < 0 {
            let call_result = _call_result.abs() as usize;
            let mut returndata = to_arraybuffer_layout(&vec![0; call_result]);
            unsafe {
                __returndatacopy(to_passback_ptr(&mut returndata));
            }
            let response = CallResponse::parse(&mut Cursor::new((&returndata[4..]).to_vec()))?;
            if response.data.len() <= 4 || &response.data[0..4] != &[0x08, 0xc3, 0x79, 0xa0] {
                return Err(anyhow!("Extcall failed (no details available)"));
            }
            let error_message = String::from_utf8_lossy(&response.data[4..]).to_string();
            return Err(anyhow!("Extcall failed: {}", error_message));
        } else {
            let call_result = _call_result as usize;
            let mut returndata = to_arraybuffer_layout(&vec![0; call_result]);
            unsafe {
                __returndatacopy(to_passback_ptr(&mut returndata));
            }
            let response = CallResponse::parse(&mut Cursor::new((&returndata[4..]).to_vec()))?;
            Ok(response)
        }
    }
}

pub struct Call(());

impl Extcall for Call {
    fn __call(cellpack: i32, outgoing_alkanes: i32, checkpoint: i32, fuel: u64) -> i32 {
        unsafe { __call(cellpack, outgoing_alkanes, checkpoint, fuel) }
    }
}

pub struct Delegatecall(());

impl Extcall for Delegatecall {
    fn __call(cellpack: i32, outgoing_alkanes: i32, checkpoint: i32, fuel: u64) -> i32 {
        unsafe { __delegatecall(cellpack, outgoing_alkanes, checkpoint, fuel) }
    }
}

pub struct Staticcall(());

impl Extcall for Staticcall {
    fn __call(cellpack: i32, outgoing_alkanes: i32, checkpoint: i32, fuel: u64) -> i32 {
        unsafe { __staticcall(cellpack, outgoing_alkanes, checkpoint, fuel) }
    }
}

pub trait AlkaneResponder: 'static {
    fn context(&self) -> Result<Context> {
        unsafe {
            let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; __request_context() as usize]);
            __load_context(to_ptr(&mut buffer) + 4);
            let res = Context::parse(&mut Cursor::<Vec<u8>>::new((&buffer[4..]).to_vec()));
            res
        }
    }
    fn block(&self) -> Vec<u8> {
        unsafe {
            let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; __request_block() as usize]);
            __load_block(to_ptr(&mut buffer) + 4);
            (&buffer[4..]).to_vec()
        }
    }
    #[allow(static_mut_refs)]
    fn initialize(&self) -> &Self {
        unsafe {
            if _CACHE.is_none() {
                initialize_cache();
                #[cfg(feature = "panic-hook")]
                panic::set_hook(Box::new(panic_hook));
            }
            self
        }
    }
    fn transaction(&self) -> Vec<u8> {
        unsafe {
            let mut buffer: Vec<u8> =
                to_arraybuffer_layout(vec![0; __request_transaction() as usize]);
            __load_transaction(to_ptr(&mut buffer) + 4);
            (&buffer[4..]).to_vec()
        }
    }
    /*
    fn output(&self, v: &OutPoint) -> Result<Vec<u8>> {
        let mut buffer = to_arraybuffer_layout(consensus_encode(v)?);
        let serialized = to_passback_ptr(&mut buffer);
        unsafe {
            let mut result: Vec<u8> =
                to_arraybuffer_layout(vec![0; __request_output(serialized) as usize]);
            let sz = __load_output(serialized, to_passback_ptr(&mut result));
            if sz == i32::MAX {
              Err(anyhow!("error fetching output"))
            } else if sz == 0 {
              Err(anyhow!("output not found"))
            } else {
              Ok((&result[4..]).to_vec())
            }
        }
    }
    */
    #[allow(static_mut_refs)]
    fn load(&self, k: Vec<u8>) -> Vec<u8> {
        unsafe {
            initialize_cache();
            if _CACHE.as_ref().unwrap().0.contains_key(&k) {
                _CACHE
                    .as_ref()
                    .unwrap()
                    .get(&k)
                    .map(|v| v.clone())
                    .unwrap_or_else(|| Vec::<u8>::new())
            } else {
                let mut key_bytes = to_arraybuffer_layout(&k);
                let key = to_passback_ptr(&mut key_bytes);
                let buf_size = __request_storage(key) as usize;
                let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; buf_size]);
                __load_storage(key, to_passback_ptr(&mut buffer));
                (&buffer[4..]).to_vec()
            }
        }
    }
    #[allow(static_mut_refs)]
    fn store(&self, k: Vec<u8>, v: Vec<u8>) {
        unsafe {
            initialize_cache();
            _CACHE.as_mut().unwrap().set(&k, &v);
        }
    }
    fn balance(&self, who: &AlkaneId, what: &AlkaneId) -> u128 {
        unsafe {
            let mut who_bytes: Vec<u8> = to_arraybuffer_layout::<Vec<u8>>(who.clone().into());
            let mut what_bytes: Vec<u8> = to_arraybuffer_layout::<Vec<u8>>(what.clone().into());
            let who_ptr = to_ptr(&mut who_bytes) + 4;
            let what_ptr = to_ptr(&mut what_bytes) + 4;
            let mut output: Vec<u8> = to_arraybuffer_layout::<Vec<u8>>(vec![0u8; 16]);
            __balance(who_ptr, what_ptr, to_ptr(&mut output) + 4);
            u128::from_le_bytes((&output[4..]).try_into().unwrap())
        }
    }
    fn sequence(&self) -> u128 {
        unsafe {
            let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; 16]);
            __sequence(to_ptr(&mut buffer) + 4);
            u128::from_le_bytes((&buffer[4..]).try_into().unwrap())
        }
    }
    fn fuel(&self) -> u64 {
        unsafe {
            let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; 8]);
            __fuel(to_ptr(&mut buffer) + 4);
            u64::from_le_bytes((&buffer[4..]).try_into().unwrap())
        }
    }
    fn height(&self) -> u64 {
        unsafe {
            let mut buffer: Vec<u8> = to_arraybuffer_layout(vec![0; 8]);
            __height(to_ptr(&mut buffer) + 4);
            u64::from_le_bytes((&buffer[4..]).try_into().unwrap())
        }
    }
    fn extcall<T: Extcall>(
        &self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        T::call(cellpack, outgoing_alkanes, fuel)
    }
    fn call(
        &self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Call>(cellpack, outgoing_alkanes, fuel)
    }
    fn delegatecall(
        &self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Delegatecall>(cellpack, outgoing_alkanes, fuel)
    }
    fn staticcall(
        &self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Staticcall>(cellpack, outgoing_alkanes, fuel)
    }
    #[allow(static_mut_refs)]
    fn run(&self) -> Vec<u8> {
        let extended: ExtendedCallResponse = match self.initialize().execute() {
            Ok(v) => handle_success(v),
            Err(e) => handle_error(&e.to_string()),
        };
        extended.serialize()
    }
    #[allow(static_mut_refs)]
    fn run_and_forward(&self) -> Vec<u8> {
        let context = self.context().unwrap();
        let extended: ExtendedCallResponse = match self.initialize().execute() {
            Ok(v) => {
                let mut response: ExtendedCallResponse = v.into();
                initialize_cache();
                response.storage = unsafe { _CACHE.as_ref().unwrap().clone() };
                response
                    .alkanes
                    .0
                    .append(&mut context.incoming_alkanes.0.clone());
                response
            }
            Err(e) => {
                let mut response = CallResponse::default();
                let mut data: Vec<u8> = vec![0x08, 0xc3, 0x79, 0xa0];
                data.extend(e.to_string().as_bytes());
                response.data = data;
                _abort();
                response.into()
            }
        };
        extended.serialize()
    }
    fn execute(&self) -> Result<CallResponse>;
}
