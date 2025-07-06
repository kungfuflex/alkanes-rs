//! Virtual Machine implementations for alkanes protocol
//! 
//! This module provides generic VM infrastructure that can work with different
//! KeyValuePointer backends, enabling the same message processing logic to run
//! in both the main indexer and GPU pipeline contexts.

pub mod wasmi;

use crate::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::AlkaneTransferParcel,
    response::ExtendedCallResponse,
    storage::StorageMap,
    trace::{Trace, TraceContext, TraceEvent, TraceResponse},
    witness::find_witness_payload,
    gz::decompress,
};
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint, Transaction};
use metashrew_support::{
    index_pointer::KeyValuePointer,
    byte_view::ByteView,
};
use protorune_support::{
    balance_sheet::{BalanceSheet, BalanceSheetOperations},
    rune_transfer::RuneTransfer,
    utils::{consensus_encode, decode_varint_list},
};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

// Implement ByteView for AlkaneId so it can be used with KeyValuePointer
impl ByteView for AlkaneId {
    fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&self.block.to_le_bytes());
        result.extend_from_slice(&self.tx.to_le_bytes());
        result
    }
    
    fn from_bytes(bytes: Vec<u8>) -> Self {
        if bytes.len() >= 16 {
            let block = u128::from_le_bytes(bytes[0..16].try_into().unwrap_or([0; 16]));
            let tx = if bytes.len() >= 32 {
                u128::from_le_bytes(bytes[16..32].try_into().unwrap_or([0; 16]))
            } else {
                0
            };
            AlkaneId { block, tx }
        } else {
            AlkaneId::default()
        }
    }
    
    fn zero() -> Self {
        AlkaneId::default()
    }
    
    fn maximum() -> Self {
        AlkaneId { block: u128::MAX, tx: u128::MAX }
    }
}

/// Generic message context parcel that works with any KeyValuePointer
#[derive(Clone)]
pub struct GenericMessageContextParcel<KV: KeyValuePointer + Clone> {
    pub transaction: Transaction,
    pub txindex: u32,
    pub height: u64,
    pub vout: u32,
    pub pointer: u32,
    pub refund_pointer: u32,
    pub calldata: Vec<u8>,
    pub atomic: KV,
    pub runtime_balances: Arc<BalanceSheet<KV>>,
    pub runes: Vec<RuneTransfer>,
}

/// Generic runtime context that works with any KeyValuePointer
#[derive(Clone)]
pub struct GenericAlkanesRuntimeContext<KV: KeyValuePointer + Clone> {
    pub myself: AlkaneId,
    pub caller: AlkaneId,
    pub incoming_alkanes: AlkaneTransferParcel,
    pub returndata: Vec<u8>,
    pub inputs: Vec<u128>,
    pub message: Box<GenericMessageContextParcel<KV>>,
    pub trace: Trace,
}

impl<KV: KeyValuePointer + Clone> GenericAlkanesRuntimeContext<KV> {
    pub fn from_parcel_and_cellpack(
        message: &GenericMessageContextParcel<KV>,
        cellpack: &Cellpack,
    ) -> Self {
        let cloned = cellpack.clone();
        let message_copy = message.clone();
        let incoming_alkanes = message_copy.runes.clone().into();
        Self {
            message: Box::new(message_copy),
            returndata: vec![],
            incoming_alkanes,
            myself: AlkaneId::default(),
            caller: AlkaneId::default(),
            trace: Trace::default(),
            inputs: cloned.inputs,
        }
    }
    
    pub fn flatten(&self) -> Vec<u128> {
        let mut result = Vec::<u128>::new();
        result.push(self.myself.block);
        result.push(self.myself.tx);
        result.push(self.caller.block);
        result.push(self.caller.tx);
        result.push(self.message.vout as u128);
        result.push(self.incoming_alkanes.0.len() as u128);
        for incoming in &self.incoming_alkanes.0 {
            result.push(incoming.id.block);
            result.push(incoming.id.tx);
            result.push(incoming.value);
        }
        for input in self.inputs.clone() {
            result.push(input);
        }
        result
    }
    
    pub fn flat(&self) -> crate::context::Context {
        crate::context::Context {
            myself: self.myself.clone(),
            caller: self.caller.clone(),
            vout: self.message.vout,
            incoming_alkanes: self.incoming_alkanes.clone(),
            inputs: self.inputs.clone(),
        }
    }
}

/// Generic fuel tank for gas management
pub struct GenericFuelTank {
    fuel: u64,
    #[allow(dead_code)]
    height: u32,
    #[allow(dead_code)]
    txindex: u32,
}

impl GenericFuelTank {
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            fuel: initial_fuel,
            height: 0,
            txindex: 0,
        }
    }
    
    pub fn consume_fuel(&mut self, amount: u64) -> Result<()> {
        if self.fuel < amount {
            return Err(anyhow!("Insufficient fuel: {} required, {} available", amount, self.fuel));
        }
        self.fuel -= amount;
        Ok(())
    }
    
    pub fn remaining_fuel(&self) -> u64 {
        self.fuel
    }
}

/// Generic message handler that works with any KeyValuePointer implementation
pub struct GenericAlkaneMessageHandler<KV: KeyValuePointer + Clone> {
    _phantom: std::marker::PhantomData<KV>,
}

impl<KV: KeyValuePointer + Clone> GenericAlkaneMessageHandler<KV> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Handle a message using the generic infrastructure
    pub fn handle_message(
        &self,
        parcel: &GenericMessageContextParcel<KV>,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<KV>)> {
        let cellpack: Cellpack =
            decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?.try_into()?;

        let target = cellpack.target.clone();
        let context = Arc::new(Mutex::new(GenericAlkanesRuntimeContext::from_parcel_and_cellpack(
            parcel, &cellpack,
        )));
        
        let mut atomic = parcel.atomic.keyword("/alkanes");
        let (caller, myself, binary) = self.run_special_cellpacks(context.clone(), &cellpack)?;

        // Credit balances
        self.credit_balances(&mut atomic, &myself, &parcel.runes)?;
        self.prepare_context(context.clone(), &caller, &myself, false);
        
        let txsize = parcel.transaction.vsize() as u64;
        let mut fuel_tank = GenericFuelTank::new(1000000); // Default fuel allocation
        fuel_tank.consume_fuel(txsize)?; // Consume fuel for transaction size
        
        let fuel = fuel_tank.remaining_fuel();
        let inner = context.lock().unwrap().flat();
        let trace = context.lock().unwrap().trace.clone();
        trace.clock(TraceEvent::EnterCall(TraceContext {
            inner,
            target,
            fuel,
        }));
        
        // Execute the contract (this would need VM implementation)
        let response = self.run_after_special(context.clone(), binary, fuel)?;
        
        // Process response
        self.pipe_storagemap_to(
            &response.storage,
            &mut atomic.keyword("/alkanes").select_value(myself),
        );
        
        let combined = parcel.runtime_balances.as_ref().clone();
        let alkanes_vec: Vec<RuneTransfer> = response.alkanes.clone().into();
        let mut sheet = BalanceSheet::<KV>::try_from(alkanes_vec)?;
        combined.pipe(&mut sheet)?;
        
        self.debit_balances(&mut atomic, &myself, &response.alkanes)?;
        
        let response_alkanes = response.alkanes.clone();
        trace.clock(TraceEvent::ReturnContext(TraceResponse {
            inner: response.into(),
            fuel_used: fuel - fuel_tank.remaining_fuel(),
        }));
        
        Ok((response_alkanes.into(), combined))
    }
    
    /// Run special cellpack operations (CREATE, CREATERESERVED, etc.)
    fn run_special_cellpacks(
        &self,
        context: Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        cellpack: &Cellpack,
    ) -> Result<(AlkaneId, AlkaneId, Arc<Vec<u8>>)> {
        let mut payload = cellpack.clone();
        let mut binary = Arc::<Vec<u8>>::new(vec![]);
        let mut next_sequence_pointer = self.sequence_pointer(&context.lock().unwrap().message.atomic);
        let next_sequence = next_sequence_pointer.get_value::<u128>();
        let original_target = cellpack.target.clone();
        
        if cellpack.target.is_created(next_sequence) {
            // Contract already created, load the wasm from the index
            let wasm_payload = context
                .lock()
                .unwrap()
                .message
                .atomic
                .keyword("/alkanes")
                .select_value(payload.target.clone())
                .get();
            binary = Arc::new(decompress(wasm_payload.as_ref().clone())?);
        } else if cellpack.target.is_create() {
            // Contract not created, create it by first loading the wasm from the witness
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
                .keyword("/alkanes")
                .select_value(payload.target.clone());
            pointer.set(wasm_payload.clone());
            binary = Arc::new(decompress(wasm_payload.as_ref().clone())?);
            next_sequence_pointer.set_value(next_sequence + 1);
            
            self.set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
        } else if let Some(number) = cellpack.target.reserved() {
            // Handle CREATERESERVED
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
                .keyword("/alkanes")
                .select_value(payload.target.clone());
            if ptr.get().as_ref().len() == 0 {
                ptr.set(wasm_payload.clone());
                self.set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
            } else {
                return Err(anyhow!(format!(
                    "used CREATERESERVED cellpack but {} already holds a binary",
                    number
                )));
            }
            binary = Arc::new(decompress(wasm_payload.clone().as_ref().clone())?);
        } else if let Some(factory) = cellpack.target.factory() {
            // Handle factory creation
            payload.target = AlkaneId::new(2, next_sequence);
            next_sequence_pointer.set_value(next_sequence + 1);
            let context_binary: Vec<u8> = context
                .lock()
                .unwrap()
                .message
                .atomic
                .keyword("/alkanes")
                .select_value(factory.clone())
                .get()
                .as_ref()
                .clone();
            let rc = Arc::new(context_binary);
            context
                .lock()
                .unwrap()
                .message
                .atomic
                .keyword("/alkanes")
                .select_value(payload.target.clone())
                .set(rc.clone());
            self.set_alkane_id_to_tx_id(context.clone(), &payload.target)?;
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
    
    /// Execute the contract after special cellpack processing
    fn run_after_special(
        &self,
        _context: Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        _binary: Arc<Vec<u8>>,
        _start_fuel: u64,
    ) -> Result<ExtendedCallResponse> {
        // This would be implemented by the specific VM (WASM for main indexer, GPU for pipeline)
        // For now, return a default response
        Ok(ExtendedCallResponse::default())
    }
    
    /// Helper functions
    fn sequence_pointer(&self, ptr: &KV) -> KV {
        ptr.keyword("/alkanes/sequence")
    }
    
    fn set_alkane_id_to_tx_id(
        &self,
        context: Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        alkane_id: &AlkaneId,
    ) -> Result<()> {
        let context_guard = context.lock().unwrap();
        let outpoint = OutPoint {
            txid: context_guard.message.transaction.compute_txid(),
            vout: context_guard.message.vout,
        };
        let outpoint_bytes: Vec<u8> = consensus_encode(&outpoint)?;
        
        let mut ptr = context_guard
            .message
            .atomic
            .keyword("/alkanes_id_to_outpoint")
            .select_value(alkane_id.clone());
        ptr.set(Arc::new(outpoint_bytes));
        
        Ok(())
    }
    
    fn credit_balances(
        &self,
        _atomic: &mut KV,
        _myself: &AlkaneId,
        _runes: &[RuneTransfer],
    ) -> Result<()> {
        // Implementation would depend on the specific KV backend
        Ok(())
    }
    
    fn debit_balances(
        &self,
        _atomic: &mut KV,
        _myself: &AlkaneId,
        _alkanes: &AlkaneTransferParcel,
    ) -> Result<()> {
        // Implementation would depend on the specific KV backend
        Ok(())
    }
    
    fn prepare_context(
        &self,
        context: Arc<Mutex<GenericAlkanesRuntimeContext<KV>>>,
        caller: &AlkaneId,
        myself: &AlkaneId,
        _delegate: bool,
    ) {
        let mut inner = context.lock().unwrap();
        inner.caller = caller.clone();
        inner.myself = myself.clone();
    }
    
    fn pipe_storagemap_to(&self, _storage: &StorageMap, _atomic: &mut KV) {
        // Implementation would depend on the specific KV backend
    }
}

// Re-export WASM VM implementation
pub use wasmi::{WasmiAlkaneVM, WasmiHostFunctions};

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::BTreeMap;
    
    // Mock KeyValuePointer for testing using BTreeMap
    #[derive(Clone)]
    pub struct MockKV {
        data: Arc<Mutex<BTreeMap<Vec<u8>, Arc<Vec<u8>>>>>,
        path: Vec<u8>,
    }
    
    impl MockKV {
        pub fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(BTreeMap::new())),
                path: Vec::new(),
            }
        }
    }
    
    impl KeyValuePointer for MockKV {
        fn wrap(word: &Vec<u8>) -> Self {
            Self {
                data: Arc::new(Mutex::new(BTreeMap::new())),
                path: word.clone(),
            }
        }
        
        fn unwrap(&self) -> Arc<Vec<u8>> {
            Arc::new(self.path.clone())
        }
        
        fn inherits(&mut self, from: &Self) {
            self.data = from.data.clone();
        }
        
        fn get(&self) -> Arc<Vec<u8>> {
            self.data.lock().unwrap()
                .get(&self.path)
                .cloned()
                .unwrap_or_else(|| Arc::new(Vec::new()))
        }
        
        fn set(&mut self, value: Arc<Vec<u8>>) {
            self.data.lock().unwrap().insert(self.path.clone(), value);
        }
    }
    
    #[test]
    fn test_generic_message_handler() {
        let handler = GenericAlkaneMessageHandler::<MockKV>::new();
        assert!(handler._phantom == std::marker::PhantomData);
    }
    
    #[test]
    fn test_mock_kv() {
        let mut kv = MockKV::new();
        kv.set_value(42u128);
        assert_eq!(kv.get_value::<u128>(), 42);
    }
    
    #[test]
    fn test_alkane_id_byteview() {
        let alkane_id = AlkaneId { block: 123, tx: 456 };
        let bytes = alkane_id.to_bytes();
        let restored = AlkaneId::from_bytes(bytes);
        assert_eq!(alkane_id.block, restored.block);
        assert_eq!(alkane_id.tx, restored.tx);
    }
}