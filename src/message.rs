use {
	crate::{
		alkane_log,
		logging::{record_fuel_consumed, record_protostone_run, record_protostone_with_cellpack},
		network::{genesis::GENESIS_BLOCK, is_active},
		trace::save_trace,
		utils::{credit_balances, debit_balances, pipe_storagemap_to},
		vm::{
			fuel::{FuelTank, VirtualFuelBytes},
			runtime::AlkanesRuntimeContext,
			utils::{prepare_context, run_after_special, run_special_cellpacks},
		},
	},
	alkanes_support::{
		cellpack::Cellpack,
		response::ExtendedCallResponse,
		trace::{TraceContext, TraceEvent, TraceResponse},
	},
	anyhow::{anyhow, Result},
	bitcoin::OutPoint,
	metashrew_core::{
		println,
		stdio::stdout,
	},
	metashrew_support::{AtomicPointer, IndexPointer, KeyValuePointer},
	protorune::{
		balance_sheet::MintableDebit,
		message::{MessageContext, MessageContextParcel},
		protorune_init::index_unique_protorunes,
	},
	protorune_support::{
		balance_sheet::{BalanceSheet, BalanceSheetOperations},
		rune_transfer::RuneTransfer,
		utils::decode_varint_list,
	},
	std::{
		fmt::Write,
		io::Cursor,
		sync::{Arc, Mutex},
	},
};

#[derive(Clone, Default)]
pub struct AlkaneMessageContext(());

// TODO: import MessageContextParcel

pub fn handle_message(
    parcel: &MessageContextParcel,
) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
    let cellpack: Cellpack =
        decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?.try_into()?;

    // Record protostone with cellpack if it has payload
    // Note: record_protostone_run() is now called via on_protostone_processed() for ALL protostones
    if !parcel.calldata.is_empty() {
        record_protostone_with_cellpack();
    }

    // Log cellpack information only with --features logs
    alkane_log!(
        "Transaction {}: target=[{},{}], opcode={}, inputs={:?}, txid={}",
        parcel.txindex,
        cellpack.target.block,
        cellpack.target.tx,
        if !cellpack.inputs.is_empty() { cellpack.inputs[0] } else { 0 },
        cellpack.inputs,
        parcel.transaction.compute_txid()
    );

    let target = cellpack.target.clone();
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(
        parcel, &cellpack,
    )));
    let mut atomic = parcel.atomic.derive(&IndexPointer::default());
    let (caller, myself, binary) = run_special_cellpacks(context.clone(), &cellpack)?;

    // Log resolved contract addresses only with --features logs
    alkane_log!(
        "Resolved: caller=[{},{}], target=[{},{}], runes={:?}",
        caller.block, caller.tx, myself.block, myself.tx, parcel.runes
    );

    credit_balances(&mut atomic, &myself, &parcel.runes)?;
    prepare_context(context.clone(), &caller, &myself, false);
    let txsize = parcel.transaction.vfsize() as u64;
    if FuelTank::is_top() {
        FuelTank::fuel_transaction(txsize, parcel.txindex, parcel.height as u32);
    } else if FuelTank::should_advance(parcel.txindex) {
        FuelTank::refuel_block();
        FuelTank::fuel_transaction(txsize, parcel.txindex, parcel.height as u32);
    }
    let fuel = FuelTank::start_fuel();
    // NOTE: we  want to keep unwrap for cases where we lock a mutex guard,
    // it's better if it panics, so then metashrew will retry that block again
    // whereas if we do .map_err(|e| anyhow!("Mutex lock poisoned: {}", e))?
    // it could produce inconsistent indexes if the unlocking fails due to concurrency problem
    // but may pass on retry
    let inner = context.lock().unwrap().flat();
    let trace = context.lock().unwrap().trace.clone();
    trace.clock(TraceEvent::EnterCall(TraceContext {
        inner,
        target,
        fuel,
    }));
    run_after_special(context.clone(), binary, fuel)
        .and_then(|(response, gas_used)| {
            FuelTank::consume_fuel(gas_used)?;
            
            // Record fuel consumption for block statistics
            record_fuel_consumed(gas_used);
            
            pipe_storagemap_to(
                &response.storage,
                &mut atomic.derive(
                    &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                ),
            );
            let mut combined = parcel.runtime_balances.as_ref().clone();
            <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(
                parcel.runes.clone(),
            )?
            .pipe(&mut combined)?;
            let sheet = <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(
                response.alkanes.clone().into(),
            )?;
            combined.debit_mintable(&sheet, &mut atomic)?;
            debit_balances(&mut atomic, &myself, &response.alkanes)?;
            let cloned = context.clone().lock().unwrap().trace.clone();
            let response_alkanes = response.alkanes.clone();
            cloned.clock(TraceEvent::ReturnContext(TraceResponse {
                inner: response.into(),
                fuel_used: gas_used,
            }));
            save_trace(
                &OutPoint {
                    txid: parcel.transaction.compute_txid(),
                    vout: parcel.vout,
                },
                parcel.height,
                trace.clone(),
            )?;

            alkane_log!("Transaction {} completed successfully, fuel used: {}", parcel.txindex, gas_used);
            Ok((response_alkanes.into(), combined))
        })
        .or_else(|e| {
            // Log error information only with --features logs
            alkane_log!(
                "Transaction {} failed: target=[{},{}] -> [{},{}], error: {}",
                parcel.txindex,
                cellpack.target.block, cellpack.target.tx,
                myself.block, myself.tx,
                e
            );

            FuelTank::drain_fuel();
            let mut response = ExtendedCallResponse::default();

            response.data = vec![0x08, 0xc3, 0x79, 0xa0];
            response.data.extend(e.to_string().as_bytes());
            let cloned = context.clone().lock().unwrap().trace.clone();
            cloned.clock(TraceEvent::RevertContext(TraceResponse {
                inner: response,
                fuel_used: u64::MAX,
            }));
            save_trace(
                &OutPoint {
                    txid: parcel.transaction.compute_txid(),
                    vout: parcel.vout,
                },
                parcel.height,
                cloned,
            )?;
            Err(e)
        })
}

impl MessageContext for AlkaneMessageContext {
    fn protocol_tag() -> u128 {
        1
    }
    fn handle(
        _parcel: &MessageContextParcel,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<AtomicPointer>)> {
        if is_active(_parcel.height) {
            match handle_message(_parcel) {
                Ok((outgoing, runtime)) => Ok((outgoing, runtime)),
                Err(e) => {
                    crate::alkane_log!("{:?}", e);
                    Err(e) // Print the error
                }
            }
        } else {
            Err(anyhow!(
                "subprotocol inactive until block {}",
                GENESIS_BLOCK
            ))
        }
    }
    
    /// Called by protorune library when any protostone for subprotocol ID 1 is processed
    /// This ensures we count ALL protostones, not just those with cellpacks
    fn on_protostone_processed() {
        record_protostone_run();
    }
}
