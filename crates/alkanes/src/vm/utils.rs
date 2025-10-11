use crate::vm::{instance::AlkanesInstance, runtime::AlkanesRuntimeContext, state::AlkanesState};
use crate::utils::{pipe_storagemap_to, transfer_from};
use crate::vm::fuel::fuel_per_store_byte;
use alkanes_support::trace::TraceEvent;
use alkanes_support::{
    cellpack::Cellpack, gz::decompress, id::AlkaneId, parcel::AlkaneTransferParcel,
    response::ExtendedCallResponse, storage::StorageMap, utils::overflow_error,
    witness::find_witness_payload,
};
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};

use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::utils::consensus_encode;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub fn read_arraybuffer(data: &[u8], data_start: i32) -> Result<Vec<u8>> {
    let start = data_start
        .try_into()
        .map_err(|_| anyhow!("invalid start offset"))?;
    let len_bytes = data
        .get(start - 4..start)
        .ok_or_else(|| anyhow!("failed to read length prefix"))?;
    let len: usize = u32::from_le_bytes(len_bytes.try_into()?)
        .try_into()
        .map_err(|_| anyhow!("invalid length"))?;

    Ok(data
        .get(start..start + len)
        .ok_or_else(|| anyhow!("invalid buffer range"))?
        .to_vec())
}

pub fn get_memory<'a, E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<'a, E>>) -> Result<Memory> {
    caller
        .get_export("memory")
        .ok_or(anyhow!("export was not memory region"))?
        .into_memory()
        .ok_or(anyhow!("export was not memory region"))
}

pub fn sequence_pointer<E: RuntimeEnvironment + Clone>(ptr: &AtomicPointer<E>) -> AtomicPointer<E> {
    ptr.derive(&IndexPointer::from_keyword("/alkanes/sequence"))
}

fn set_alkane_id_to_tx_id<E: RuntimeEnvironment + Clone>(
    context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    alkane_id: &AlkaneId,
    env: &mut E,
) -> Result<()> {
    // Acquire the mutex once and keep the guard for the duration of the function
    let context_guard = context.lock().unwrap();

    let outpoint = OutPoint {
        txid: context_guard.message.transaction.compute_txid(),
        vout: context_guard.message.vout,
    };
    let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;

    context_guard
        .message
        .atomic
        .keyword("/alkanes_id_to_outpoint/")
        .select(&alkane_id.clone().into())
        .set(env, Arc::new(outpoint_bytes));

    Ok(())
}

pub fn get_alkane_binary<E: RuntimeEnvironment + Clone>(
    context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    alkane_id: &AlkaneId,
    env: &mut E,
) -> Result<Arc<Vec<u8>>> {
    let wasm_payload_arc = context
        .lock()
        .unwrap()
        .message
        .atomic
        .keyword("/alkanes/")
        .select(&alkane_id.clone().into())
        .get(env);
    let wasm_payload = wasm_payload_arc.as_ref();
    if wasm_payload.len() == 32 {
        let factory_id = wasm_payload.to_vec().try_into()?;
        return get_alkane_binary(context, &factory_id, env);
    }
    Ok(Arc::new(decompress(wasm_payload.clone())?))
}

pub fn run_special_cellpacks<E: RuntimeEnvironment + Clone>(
    context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    cellpack: &Cellpack,
    env: &mut E,
) -> Result<(AlkaneId, AlkaneId, Arc<Vec<u8>>)> {
    let mut payload = cellpack.clone();
    let mut binary = Arc::<Vec<u8>>::new(vec![]);
    let mut next_sequence_pointer = sequence_pointer(&mut context.lock().unwrap().message.atomic);
    let next_sequence = next_sequence_pointer.get_value::<u128>(env);
    let original_target = cellpack.target.clone();
    if cellpack.target.is_created(next_sequence) {
        binary = get_alkane_binary(context.clone(), &payload.target, env)?;
    } else if cellpack.target.is_create() {
        // contract not created, create it by first loading the wasm from the witness
        // then storing it in the index.
        let wasm_payload = Arc::new(
            find_witness_payload(&context.lock().unwrap().message.transaction.clone(), 0)
                .ok_or("finding witness payload failed for creation of alkane")
                .map_err(|_| anyhow!("used CREATE cellpack but no binary found in witness"))?,
        );
        payload.target = AlkaneId {
            block: 2,
            tx: next_sequence,
        };
        let mut pointer = context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&payload.target.clone().into());
        pointer.set(env, wasm_payload.clone());
        binary = Arc::new(decompress(wasm_payload.as_ref().clone())?);
        next_sequence_pointer.set_value(env, next_sequence + 1);

        set_alkane_id_to_tx_id(context.clone(), &payload.target, env)?;
    } else if let Some(number) = cellpack.target.reserved() {
        // we have already reserved an alkane id, find the binary and
        // set it in the index
        let wasm_payload = Arc::new(
            find_witness_payload(&context.lock().unwrap().message.transaction.clone(), 0)
                .ok_or("finding witness payload failed for creation of alkane")
                .map_err(|_| {
                    anyhow!("used CREATERESERVED cellpack but no binary found in witness")
                })?,
        );
        payload.target = AlkaneId {
            block: 4,
            tx: number,
        };
        let mut ptr = context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&payload.target.clone().into());
        if ptr.get(env).as_ref().len() == 0 {
            ptr.set(env, wasm_payload.clone());
            set_alkane_id_to_tx_id(context.clone(), &payload.target, env)?;
        } else {
            return Err(anyhow!(format!(
                "used CREATERESERVED cellpack but {} already holds a binary",
                number
            )));
        }
        binary = Arc::new(decompress(wasm_payload.clone().as_ref().clone())?);
    } else if let Some(factory) = cellpack.target.factory() {
        // we find the factory alkane wasm and set the current alkane to the factory wasm
        payload.target = AlkaneId::new(2, next_sequence);
        next_sequence_pointer.set_value(env, next_sequence + 1);
        let factory_payload: Vec<u8> = factory.into();
        context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&payload.target.clone().into())
            .set(env, Arc::new(factory_payload));
        set_alkane_id_to_tx_id(context.clone(), &payload.target, env)?;
        binary = get_alkane_binary(context.clone(), &factory, env)?;
    }
    if &original_target != &payload.target {
        context
            .lock()
            .unwrap()
            .trace
            .clock(TraceEvent::CreateAlkane(payload.target.clone()));
    }
    Ok((
        context.lock().unwrap().myself.clone(),
        payload.target.clone(),
        binary.clone(),
    ))
}

#[derive(Clone, Debug)]
pub struct SaveableExtendedCallResponse<E: RuntimeEnvironment + Clone> {
    pub result: ExtendedCallResponse,
    pub _from: AlkaneId,
    pub _to: AlkaneId,
    _phantom: PhantomData<E>,
}

impl<E: RuntimeEnvironment + Clone> Default for SaveableExtendedCallResponse<E> {
    fn default() -> Self {
        Self {
            result: ExtendedCallResponse::default(),
            _from: AlkaneId::default(),
            _to: AlkaneId::default(),
            _phantom: PhantomData,
        }
    }
}

impl<E: RuntimeEnvironment + Clone> From<ExtendedCallResponse> for SaveableExtendedCallResponse<E> {
    fn from(v: ExtendedCallResponse) -> Self {
        let mut response = Self::default();
        response.result = v;
        response
    }
}

impl<E: RuntimeEnvironment + Clone> SaveableExtendedCallResponse<E> {
    pub(super) fn associate(&mut self, context: &AlkanesRuntimeContext<E>) {
        self._from = context.myself.clone();
        self._to = context.caller.clone();
    }
}

impl<E: RuntimeEnvironment + Clone> Saveable<E> for SaveableExtendedCallResponse<E> {
    fn from(&self) -> AlkaneId {
        self._from.clone()
    }
    fn to(&self) -> AlkaneId {
        self._to.clone()
    }
    fn storage_map(&self) -> StorageMap {
        self.result.storage.clone()
    }
    fn alkanes(&self) -> AlkaneTransferParcel {
        self.result.alkanes.clone()
    }
}

pub trait Saveable<E: RuntimeEnvironment + Clone> {
    fn from(&self) -> AlkaneId;
    fn to(&self) -> AlkaneId;
    fn storage_map(&self) -> StorageMap;
    fn alkanes(&self) -> AlkaneTransferParcel;
    fn save(
        &self,
        atomic: &mut AtomicPointer<E>,
        is_delegate: bool,
        env: &mut E,
    ) -> Result<()> {
        pipe_storagemap_to(
            &self.storage_map(),
            &mut atomic
                .derive(&IndexPointer::from_keyword("/alkanes/").select(&self.from().into())),
            env,
        );
        if !is_delegate {
            // delegate call retains caller and myself, so no alkanes are transferred from the subcontext to myself
            transfer_from(
                &self.alkanes(),
                &mut atomic.derive(&IndexPointer::default()),
                &self.from().into(),
                &self.to().into(),
                env,
            )?;
        }
        Ok(())
    }
}

pub fn run_after_special<'a, E: RuntimeEnvironment + Clone + 'static + Default>(
    context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    binary: Arc<Vec<u8>>,
    start_fuel: u64,
    env: &'a mut E,
) -> Result<(ExtendedCallResponse, u64)> {
    #[cfg(feature = "debug-log")]
    {
        // Log initial fuel allocation
        println!(
            "Starting WebAssembly execution with {} fuel units",
            start_fuel
        );
    }

    let mut instance = AlkanesInstance::from_alkane(context.clone(), binary.clone(), start_fuel, env)?;
    let response = instance.execute()?;

    let remaining_fuel = instance.store.get_fuel()?;
    let storage_len = response.storage.serialize().len() as u64;
    let height = context.lock().unwrap().message.height as u32;

    #[cfg(feature = "debug-log")]
    {
        // Log fuel usage details
        println!("WebAssembly execution completed:");
        println!("  - Initial fuel: {}", start_fuel);
        println!("  - Remaining fuel: {}", remaining_fuel);
        println!("  - Direct consumption: {}", start_fuel - remaining_fuel);
        println!("  - Storage size: {} bytes", storage_len);
    }

    #[cfg(feature = "debug-log")]
    {
        // Log storage fuel cost
        let computed_storage_fuel = fuel_per_store_byte(height)
            .checked_mul(storage_len)
            .unwrap_or(0);
        println!("  - Storage fuel cost: {}", computed_storage_fuel);
    }

    let fuel_used = overflow_error(start_fuel.checked_sub(remaining_fuel).and_then(
        |v: u64| -> Option<u64> {
            let computed_fuel =
                overflow_error(fuel_per_store_byte(height).checked_mul(storage_len)).ok()?;
            let opt = v.checked_add(computed_fuel);
            #[cfg(feature = "debug-log")]
            {
                // Log total fuel used
                println!("  - Total fuel used: {}", opt.unwrap_or(u64::MAX));
            }
            opt
        },
    ))?;

    Ok((response, fuel_used))
}
pub fn prepare_context<E: RuntimeEnvironment + Clone>(
    context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    caller: &AlkaneId,
    myself: &AlkaneId,
    delegate: bool,
) {
    if !delegate {
        let mut inner = context.lock().unwrap();
        inner.caller = caller.clone();
        inner.myself = myself.clone();
    }
}

pub fn send_to_arraybuffer<'a, E: RuntimeEnvironment + Clone>(
    caller: &mut Caller<'_, AlkanesState<'a, E>>,
    ptr: usize,
    v: &Vec<u8>,
) -> Result<i32> {
    let mem = get_memory(caller)?;
    mem.write(&mut *caller, ptr - 4, &v.len().to_le_bytes())
        .map_err(|_| anyhow!("failed to write ArrayBuffer"))?;
    mem.write(&mut *caller, ptr, v.as_slice())
        .map_err(|_| anyhow!("failed to write ArrayBuffer"))?;
    Ok(ptr.try_into()?)
}
