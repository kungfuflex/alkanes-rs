
#[allow(unused_imports)]
use crate::imports::{
    __balance, __call, __delegatecall, __fuel, __height, __load_block, __load_context,
    __load_storage, __load_transaction, __log, __request_block, __request_context,
    __request_storage, __request_transaction, __returndatacopy, __sequence, __staticcall,
    abort, /*, __load_output, __request_output */
};
use crate::storage::StoragePointer;
#[allow(unused_imports)]
use crate::{
    println,
    stdio::{stdout, Write},
};
use anyhow::{anyhow, Result};
use bitcoin::{block::Header, Transaction, Txid};
#[allow(unused_imports)]
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr, to_ptr};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_decode};
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



#[allow(static_mut_refs)]
pub fn handle_success(response: CallResponse, env: &mut AlkaneEnvironment) -> ExtendedCallResponse {
    let mut extended: ExtendedCallResponse = response.into();
    let storage = StorageMap(
        env.cache
            .iter()
            .map(|(k, v)| ((**k).clone(), (**v).clone()))
            .collect(),
    );
    extended.storage = storage;
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
        env: &mut AlkaneEnvironment,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        let mut cellpack_buffer = to_arraybuffer_layout::<&[u8]>(&cellpack.serialize());
        let mut outgoing_alkanes_buffer: Vec<u8> =
            to_arraybuffer_layout::<&[u8]>(&outgoing_alkanes.serialize());
        let storage = StorageMap(
            env.cache
                .iter()
                .map(|(k, v)| ((**k).clone(), (**v).clone()))
                .collect(),
        );
        let mut storage_map_buffer = to_arraybuffer_layout::<&[u8]>(&storage.serialize());
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
            if returndata.len() < 20 {
                return Err(anyhow!(format!(
                    "Extcall failed, and returndatacopy len ({}) < AlkanesTransferParcel min size 20 ",
                    returndata.len()
                )));
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
            if returndata.len() < 20 {
                return Err(anyhow!(format!(
                    "Extcall succeeded, but returndatacopy len ({}) < AlkanesTransferParcel min size 20 ",
                    returndata.len()
                )));
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

pub use crate::environment::AlkaneEnvironment;

pub trait AlkaneResponder: 'static {
    fn env(&mut self) -> &mut AlkaneEnvironment;
    fn observe_initialization(&mut self) -> Result<()> {
        let mut pointer: StoragePointer = KeyValuePointer::<AlkaneEnvironment>::from_keyword("/initialized");
        if pointer.get(self.env()).len() == 0 {
            pointer.set_value::<u8>(self.env(), 0x01);
            Ok(())
        } else {
            Err(anyhow!("already initialized"))
        }
    }
    fn observe_proxy_initialization(&mut self) -> Result<()> {
        let mut pointer: StoragePointer = KeyValuePointer::<AlkaneEnvironment>::from_keyword("/proxy_initialized");
        if pointer.get(self.env()).len() == 0 {
            pointer.set_value::<u8>(self.env(), 0x01);
            Ok(())
        } else {
            Err(anyhow!("proxy already initialized"))
        }
    }
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

    fn transaction(&self) -> Vec<u8> {
        unsafe {
            let mut buffer: Vec<u8> =
                to_arraybuffer_layout(vec![0; __request_transaction() as usize]);
            __load_transaction(to_ptr(&mut buffer) + 4);
            (&buffer[4..]).to_vec()
        }
    }
    fn transaction_object(&self) -> Result<Transaction> {
        Ok(consensus_decode::<Transaction>(&mut std::io::Cursor::new(
            self.transaction(),
        ))?)
    }
    fn transaction_id(&self) -> Result<Txid> {
        Ok(self.transaction_object()?.compute_txid())
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
        &mut self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        T::call(self.env(), cellpack, outgoing_alkanes, fuel)
    }
    fn call(
        &mut self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Call>(cellpack, outgoing_alkanes, fuel)
    }
    fn delegatecall(
        &mut self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Delegatecall>(cellpack, outgoing_alkanes, fuel)
    }
    fn staticcall(
        &mut self,
        cellpack: &Cellpack,
        outgoing_alkanes: &AlkaneTransferParcel,
        fuel: u64,
    ) -> Result<CallResponse> {
        self.extcall::<Staticcall>(cellpack, outgoing_alkanes, fuel)
    }

    fn block_header(&mut self) -> Result<Header> {
        let result = self.staticcall(
            &Cellpack {
                target: AlkaneId {
                    block: 800000000,
                    tx: 0,
                },
                inputs: vec![],
            },
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        consensus_decode::<Header>(&mut std::io::Cursor::new(result.data))
    }

    fn coinbase_tx(&mut self) -> Result<Transaction> {
        let result = self.staticcall(
            &Cellpack {
                target: AlkaneId {
                    block: 800000000,
                    tx: 1,
                },
                inputs: vec![],
            },
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        consensus_decode::<Transaction>(&mut std::io::Cursor::new(result.data))
    }

    fn number_diesel_mints(&mut self) -> Result<u128> {
        let result = self.staticcall(
            &Cellpack {
                target: AlkaneId {
                    block: 800000000,
                    tx: 2,
                },
                inputs: vec![],
            },
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        Ok(u128::from_le_bytes(result.data[0..16].try_into()?))
    }

    fn total_miner_fee(&mut self) -> Result<u128> {
        let result = self.staticcall(
            &Cellpack {
                target: AlkaneId {
                    block: 800000000,
                    tx: 3,
                },
                inputs: vec![],
            },
            &AlkaneTransferParcel::default(),
            self.fuel(),
        )?;
        Ok(u128::from_le_bytes(result.data[0..16].try_into()?))
    }

    /// Fallback function that gets called when an opcode is not recognized
    ///
    /// This default implementation reverts with an error.
    /// Contracts can override this method to provide custom fallback behavior.
    fn fallback(&mut self) -> Result<CallResponse> {
        Err(anyhow!("Unrecognized opcode"))
    }
}
