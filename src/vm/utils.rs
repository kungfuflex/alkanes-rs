use super::{AlkanesInstance, AlkanesRuntimeContext, AlkanesState};
use crate::utils::{pipe_storagemap_to, transfer_from};
use crate::vm::fuel::compute_extcall_fuel;
use alkanes_support::trace::TraceEvent;
use alkanes_support::{
    cellpack::Cellpack, gz::decompress, id::AlkaneId, parcel::AlkaneTransferParcel,
    response::ExtendedCallResponse, storage::StorageMap, utils::overflow_error,
    witness::find_witness_payload,
};
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
#[allow(unused_imports)]
use metashrew_core::{
    clear as clear_base, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use protobuf::SpecialFields;
use protorune_support::utils::consensus_encode;
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

pub fn get_memory<'a>(caller: &mut Caller<'_, AlkanesState>) -> Result<Memory> {
    caller
        .get_export("memory")
        .ok_or(anyhow!("export was not memory region"))?
        .into_memory()
        .ok_or(anyhow!("export was not memory region"))
}

pub fn sequence_pointer(ptr: &AtomicPointer) -> AtomicPointer {
    ptr.derive(&IndexPointer::from_keyword("/alkanes/sequence"))
}

fn set_alkane_id_to_tx_id(
    context: Arc<Mutex<AlkanesRuntimeContext>>,
    alkane_id: &AlkaneId,
) -> Result<()> {
    let outpoint = OutPoint {
        txid: context.lock().unwrap().message.transaction.compute_txid(),
        vout: context.lock().unwrap().message.vout,
    };
    let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;
    context
        .lock()
        .unwrap()
        .message
        .atomic
        .keyword("/alkanes_id_to_outpoint/")
        .select(&alkane_id.clone().into())
        .set(Arc::new(outpoint_bytes));

    Ok(())
}

pub fn run_special_cellpacks(
    context: Arc<Mutex<AlkanesRuntimeContext>>,
    cellpack: &Cellpack,
) -> Result<(AlkaneId, AlkaneId, Arc<Vec<u8>>)> {
    let mut payload = cellpack.clone();
    let mut binary = Arc::<Vec<u8>>::new(vec![]);
    let mut next_sequence_pointer = sequence_pointer(&mut context.lock().unwrap().message.atomic);
    let next_sequence = next_sequence_pointer.get_value::<u128>();
    let original_target = cellpack.target.clone();
    if cellpack.target.is_created(next_sequence) {
        // contract already created, load the wasm from the index
        let wasm_payload = context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&payload.target.clone().into())
            .get();
        binary = Arc::new(decompress(wasm_payload.as_ref().clone())?);
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
        pointer.set(wasm_payload.clone());
        binary = Arc::new(decompress(wasm_payload.as_ref().clone())?);
        next_sequence_pointer.set_value(next_sequence + 1);

        set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
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
        if ptr.get().as_ref().len() == 0 {
            ptr.set(wasm_payload.clone());
            set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
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
        next_sequence_pointer.set_value(next_sequence + 1);
        let context_binary: Vec<u8> = context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&factory.clone().into())
            .get()
            .as_ref()
            .clone();
        let rc = Arc::new(context_binary);
        context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&payload.target.clone().into())
            .set(rc.clone()); // TODO: we don't need to store this twice
        set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
        binary = Arc::new(decompress(rc.as_ref().clone())?);
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

#[derive(Clone, Default, Debug)]
pub struct SaveableExtendedCallResponse {
    pub result: ExtendedCallResponse,
    pub _from: AlkaneId,
    pub _to: AlkaneId,
}

impl From<ExtendedCallResponse> for SaveableExtendedCallResponse {
    fn from(v: ExtendedCallResponse) -> Self {
        let mut response = Self::default();
        response.result = v;
        response
    }
}

impl SaveableExtendedCallResponse {
    pub(super) fn associate(&mut self, context: &AlkanesRuntimeContext) {
        self._from = context.myself.clone();
        self._to = context.caller.clone();
    }
}

impl Saveable for SaveableExtendedCallResponse {
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

pub trait Saveable {
    fn from(&self) -> AlkaneId;
    fn to(&self) -> AlkaneId;
    fn storage_map(&self) -> StorageMap;
    fn alkanes(&self) -> AlkaneTransferParcel;
    fn save(&self, atomic: &mut AtomicPointer) -> Result<()> {
        pipe_storagemap_to(
            &self.storage_map(),
            &mut atomic
                .derive(&IndexPointer::from_keyword("/alkanes/").select(&self.from().into())),
        );
        transfer_from(
            &self.alkanes(),
            &mut atomic.derive(&IndexPointer::default()),
            &self.from().into(),
            &self.to().into(),
        )?;
        Ok(())
    }
}

pub fn run_after_special(
    context: Arc<Mutex<AlkanesRuntimeContext>>,
    binary: Arc<Vec<u8>>,
    start_fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    #[cfg(feature = "debug-log")]
    {
        // Log initial fuel allocation
        println!(
            "Starting WebAssembly execution with {} fuel units",
            start_fuel
        );
    }

    let mut instance = AlkanesInstance::from_alkane(context.clone(), binary.clone(), start_fuel)?;
    let response = instance.execute()?;

    let remaining_fuel = instance.store.get_fuel().unwrap();
    let storage_len = response.storage.serialize().len() as u64;

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
        let computed_storage_fuel = compute_extcall_fuel(storage_len).unwrap_or(0);
        println!("  - Storage fuel cost: {}", computed_storage_fuel);
    }

    let fuel_used = overflow_error(start_fuel.checked_sub(remaining_fuel).and_then(
        |v: u64| -> Option<u64> {
            let computed_fuel = compute_extcall_fuel(storage_len).ok()?;
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
pub fn prepare_context(
    context: Arc<Mutex<AlkanesRuntimeContext>>,
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

pub fn run(
    context: Arc<Mutex<AlkanesRuntimeContext>>,
    cellpack: &Cellpack,
    start_fuel: u64,
    delegate: bool,
) -> Result<(ExtendedCallResponse, u64)> {
    let (caller, myself, binary) = run_special_cellpacks(context.clone(), cellpack)?;
    prepare_context(context.clone(), &caller, &myself, delegate);
    run_after_special(context, binary, start_fuel)
}

pub fn send_to_arraybuffer<'a>(
    caller: &mut Caller<'_, AlkanesState>,
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
