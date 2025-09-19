use crate::network::{genesis::GENESIS_BLOCK, is_active};
use crate::trace::save_trace;
use crate::utils::{credit_balances, debit_balances, pipe_storagemap_to};
use crate::vm::{
    fuel::{AlkanesTransaction, FuelTank},
    runtime::AlkanesRuntimeContext,
    utils::{prepare_context, run_after_special, run_special_cellpacks},
};
use alkanes_support::{
    cellpack::Cellpack,
    response::ExtendedCallResponse,
    trace::{TraceContext, TraceEvent, TraceResponse},
    virtual_fuel::VirtualFuelBytes,
};
use anyhow::{anyhow, Result};
use bitcoin::OutPoint;
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::environment::{EnvironmentInput, RuntimeEnvironment};


use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::MintableDebit;
use protorune::message::{MessageContext, MessageContextParcel};
#[allow(unused_imports)]
use protorune::protorune_init::index_unique_protorunes;
use protorune_support::balance_sheet::BalanceSheetOperations;
use protorune_support::{
    balance_sheet::BalanceSheet, rune_transfer::RuneTransfer, utils::decode_varint_list,
};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;

#[derive(Clone, Default, Debug)]
pub struct AlkaneMessageContext<E: RuntimeEnvironment>(PhantomData<E>);




// TODO: import MessageContextParcel

pub fn handle_message<E: RuntimeEnvironment + Clone + Default + 'static>(
    parcel: &MessageContextParcel<E>,
    env: &mut E,
) -> Result<(Vec<RuneTransfer>, BalanceSheet<E, AtomicPointer<E>>)> {
    let cellpack: Cellpack =
        decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?.try_into()?;

    #[cfg(feature = "debug-log")]
    {
        // Log cellpack information at the beginning of transaction processing
        env.log("=== TRANSACTION CELLPACK INFO ===");
        env.log(&format!(
            "Transaction index: {}, Transaction height: {}, vout: {}, txid: {}",
            parcel.txindex,
            parcel.height,
            parcel.vout,
            parcel.transaction.compute_txid()
        ));
        env.log(&format!(
            "Target contract: [block={}, tx={}]",
            cellpack.target.block, cellpack.target.tx
        ));
        env.log(&format!("Input count: {}", cellpack.inputs.len()));
        if !cellpack.inputs.is_empty() {
            env.log(&format!("First opcode: {}", cellpack.inputs[0]));

            // Print all inputs for detailed debugging
            env.log(&format!("All inputs: {:?}", cellpack.inputs));
        }
        env.log("=================================");
    }

    let target = cellpack.target.clone();
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(
        parcel, &cellpack,
    )));
    let mut atomic = parcel.atomic.derive(&IndexPointer::default());
    let (caller, myself, binary) = run_special_cellpacks(context.clone(), &cellpack, env)?;

    #[cfg(feature = "debug-log")]
    {
        // Log the resolved contract addresses
        env.log(&format!("Caller: [block={}, tx={}]", caller.block, caller.tx));
        env.log(&format!(
            "Target resolved to: [block={}, tx={}]",
            myself.block, myself.tx
        ));
        env.log(&format!("Parcel runes: {:?}", parcel.runes));
    }

    credit_balances(&mut atomic, &myself, &parcel.runes, env)?;
    prepare_context(context.clone(), &caller, &myself, false);
    let txsize = AlkanesTransaction::<E>(&parcel.transaction, PhantomData).vfsize() as u64;
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
    run_after_special(context.clone(), binary, fuel, env)
        .and_then(|(response, gas_used)| {
            FuelTank::consume_fuel(gas_used)?;
            pipe_storagemap_to(
                &response.storage,
                &mut atomic.derive(
                    &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                ),
                env,
            );
            let mut combined = parcel.runtime_balances.as_ref().clone();
            <BalanceSheet<E, AtomicPointer<E>> as TryFrom<Vec<RuneTransfer>>>::try_from(
                parcel.runes.clone(),
            )?
            .pipe(&mut combined, env)?;;
            let sheet = <BalanceSheet<E, AtomicPointer<E>> as TryFrom<Vec<RuneTransfer>>>::try_from(
                response.alkanes.clone().into(),
            )?;
            combined.debit_mintable(&sheet, &mut atomic, env)?;
            debit_balances(&mut atomic, &myself, &response.alkanes, env)?;
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
                env,
            )?;

            Ok((response_alkanes.into(), combined))
        })
        .or_else(|e| {
            #[cfg(feature = "debug-log")]
            {
                // Log detailed error information
                env.log("=== TRANSACTION ERROR ===");
                env.log(&format!("Transaction index: {}", parcel.txindex));
                env.log(&format!(
                    "Target contract: [block={}, tx={}]",
                    cellpack.target.block, cellpack.target.tx
                ));
                env.log(&format!(
                    "Resolved target: [block={}, tx={}]",
                    myself.block, myself.tx
                ));
                env.log(&format!("Error: {}", e));

                // If it's a fuel-related error, provide more context
                if e.to_string().contains("fuel") || e.to_string().contains("gas") {
                    env.log("This appears to be a fuel-related error.");
                    env.log(&format!(
                        "Contract at [block={}, tx={}] with opcode {} consumed too much fuel.",
                        myself.block,
                        myself.tx,
                        if !cellpack.inputs.is_empty() {
                            cellpack.inputs[0].to_string()
                        } else {
                            "unknown".to_string()
                        }
                    ));
                }
                env.log("========================");
            }

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
                env,
            )?;
            Err(e)
        })
}

impl<E: RuntimeEnvironment + Clone + Default + 'static> MessageContext<E> for AlkaneMessageContext<E> {
    fn protocol_tag() -> u128 {
        1
    }
    fn handle(
        parcel: &MessageContextParcel<E>,
        env: &mut E,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<E, AtomicPointer<E>>)> {
        if is_active(parcel.height) {
            match handle_message(parcel, env) {
                Ok((outgoing, runtime)) => Ok((outgoing, runtime)),
                Err(e) => {
                    env.log(&format!("{:?}", e));
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
}
