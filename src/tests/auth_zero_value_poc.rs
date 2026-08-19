//! Regression coverage for the `only_owner()` zero-value auth bypass.
//!
//! History: before commit 01885bc ("better auth token", 2025-06-22)
//! `AuthenticatedResponder::only_owner()` gated on presence alone:
//!
//! ```ignore
//! if !context.incoming_alkanes.0.iter().any(|i| i.id == auth_token) { ... }
//! ```
//!
//! It never looked at `i.value`. That is only safe if a zero-value entry can
//! never appear in `incoming_alkanes` AND the follow-up `authenticate` call
//! can never be funded from the contract's own pocket. Neither holds:
//!
//!   1. `alkanes-std-auth-token::authenticate()` returns the auth token TWICE
//!      (`CallResponse::forward(incoming)` + `push(transfer)`), and the runtime
//!      lets an alkane mint itself (`checked_debit_with_minting`). So every
//!      owner-only call net-deposits +1 auth token into the *guarded contract's*
//!      own inventory. Test 1 below pins that behaviour, which is still present.
//!
//!   2. An extcall's incoming parcel is attacker-chosen and is NOT filtered for
//!      zero values (`AlkaneTransferParcel::parse` -> `subbed.incoming_alkanes`
//!      in vm/host_functions.rs). So a contract can hand a victim
//!      `[{auth_token, 0}]`. Test 2 below drives exactly that.
//!
//! With (1) funding the `authenticate` call and (2) satisfying the presence
//! check, a caller holding zero auth tokens passed `only_owner()`.
//!
//! The top-level (edict) path was never the vector: `RuneTransfer::from_balance_sheet`
//! drops zero balances, so `incoming_alkanes` from a Bitcoin tx is always > 0.
//! Test 3 pins that.

use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{self as alkane_helpers, clear};
use crate::tests::std::{
    alkanes_std_auth_token_build, alkanes_std_owned_token_build, alkanes_std_test_build,
};
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID, id::AlkaneId};
use anyhow::Result;
use bitcoin::{Block, OutPoint, Witness};
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::balance_sheet::BalanceSheetOperations;
use protorune_support::protostone::ProtostoneEdict;
use wasm_bindgen_test::wasm_bindgen_test;

const OWNED_TOKEN: AlkaneId = AlkaneId { block: 2, tx: 1 };

const MINT_OPCODE: u128 = 77;
const EXT_CALL_WITH_TRANSFER: u128 = 111;

/// Read the auth token id that `deploy_auth_token` recorded under the guarded
/// contract's `/auth` storage key, rather than assuming a deploy sequence.
fn auth_token_of(contract: AlkaneId) -> AlkaneId {
    let contract_bytes: Vec<u8> = contract.into();
    let raw = IndexPointer::from_keyword("/alkanes/")
        .select(&contract_bytes)
        .keyword("/storage//auth")
        .get();
    AlkaneId::try_from(raw.as_ref().clone()).expect("/auth pointer should hold an AlkaneId")
}

/// Locate the `2:n` id that `binary` deployed to, rather than assuming a
/// deploy sequence (the auth-token factory and the guarded contract's own
/// auth-token deployment both consume sequence numbers).
fn find_deployed(binary: Vec<u8>) -> AlkaneId {
    let want = alkanes_support::gz::compress(binary).unwrap().len();
    for tx in 0..16u128 {
        let id = AlkaneId { block: 2, tx };
        let stored = IndexPointer::from_keyword("/alkanes/")
            .select(&id.into())
            .get();
        if stored.len() == want {
            return id;
        }
    }
    panic!("binary of compressed length {} not deployed under 2:0..2:15", want);
}

/// Find the outpoint in `block` that holds `amount` units of `token`.
fn find_outpoint_holding(block: &Block, token: AlkaneId, amount: u128) -> OutPoint {
    for tx in &block.txdata {
        for vout in 0..tx.output.len() as u32 {
            let outpoint = OutPoint {
                txid: tx.compute_txid(),
                vout,
            };
            if outpoint_sheet(&outpoint).get_cached(&token.into()) == amount {
                return outpoint;
            }
        }
    }
    panic!("no outpoint in block holds {} of {:?}", amount, token);
}

/// A contract's committed internal inventory balance of `token`. Mirrors the
/// key path built by `crate::utils::balance_pointer`.
fn alkane_inventory(holder: AlkaneId, token: AlkaneId) -> u128 {
    let token_bytes: Vec<u8> = token.into();
    let holder_bytes: Vec<u8> = holder.into();
    IndexPointer::from_keyword("/alkanes/")
        .select(&token_bytes)
        .keyword("/balances/")
        .select(&holder_bytes)
        .get_value::<u128>()
}

fn outpoint_sheet(outpoint: &OutPoint) -> protorune_support::balance_sheet::BalanceSheet<IndexPointer>
{
    load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(outpoint).unwrap()),
    )
}

/// Deploy the auth-token factory, an owned-token (which mints itself an auth
/// token at 2:2), and the LoggerAlkane test contract used as the attacker.
/// Returns the deploy block; the owner's auth token sits at its last tx, vout 0.
fn deploy(block_height: u32) -> Result<Block> {
    let auth_factory = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let init_owned_token = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0 /* Initialize */, 1 /* auth units */, 1000 /* token units */],
    };
    let init_attacker = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0 /* Initialize */],
    };

    let block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
        ],
        vec![auth_factory, init_owned_token, init_attacker],
    );
    index_block(&block, block_height)?;
    Ok(block)
}

/// The owner spends `auth_outpoint`, edicts 1 auth token into the message, and
/// calls `Mint`. Returns the new outpoint holding the owner's alkanes.
fn owner_mint(
    block_height: u32,
    auth_token: AlkaneId,
    auth_outpoint: OutPoint,
    units: u128,
) -> Result<OutPoint> {
    let mut block = protorune::test_helpers::create_block_with_coinbase_tx(block_height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in_with_edicts(
        Witness::new(),
        vec![
            alkane_helpers::CellpackOrEdict::Edict(vec![ProtostoneEdict {
                id: auth_token.into(),
                amount: 1,
                output: 0,
            }]),
            alkane_helpers::CellpackOrEdict::Cellpack(Cellpack {
                target: OWNED_TOKEN,
                inputs: vec![MINT_OPCODE, units],
            }),
        ],
        auth_outpoint,
        false,
    );
    block.txdata.push(tx.clone());
    index_block(&block, block_height)?;
    Ok(OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    })
}

/// Every successful `only_owner()` call leaks +1 auth token into the guarded
/// contract's own inventory, because `authenticate()` returns the token twice
/// and the runtime permits self-minting. This is the stash that funds the
/// bypass in `test_only_owner_rejects_zero_value_auth_transfer`.
#[wasm_bindgen_test]
fn test_only_owner_leaks_auth_token_into_guarded_contract() -> Result<()> {
    clear();
    let deploy_block = deploy(880_000)?;
    let auth_token = auth_token_of(OWNED_TOKEN);

    // Nothing banked before any owner-only call.
    assert_eq!(alkane_inventory(OWNED_TOKEN, auth_token), 0);

    // The owner holds exactly the 1 auth token minted at deploy.
    let mut auth_outpoint = find_outpoint_holding(&deploy_block, auth_token, 1);

    for i in 1..=3u128 {
        auth_outpoint = owner_mint(880_000 + i as u32, auth_token, auth_outpoint, 100)?;
        let sheet = outpoint_sheet(&auth_outpoint);
        // The owner never loses the auth token...
        assert_eq!(sheet.get_cached(&auth_token.into()), 1);
        // ...yet the guarded contract banks one more copy on every call.
        assert_eq!(
            alkane_inventory(OWNED_TOKEN, auth_token),
            i,
            "owned-token should have accumulated {} auth token(s) after {} owner-only call(s)",
            i,
            i
        );
    }

    Ok(())
}

/// The bypass itself. The attacker holds ZERO auth tokens and calls
/// `OwnedToken::Mint` with an extcall parcel of `[{AUTH_TOKEN, 0}]`.
///
/// Pre-01885bc this passed: the presence check `any(|i| i.id == auth_token)`
/// matched the zero-value entry, and the follow-up `authenticate` call was
/// funded from the stash accumulated in the test above.
///
/// Post-fix the `i.value > 0` clause rejects it. If this test ever fails,
/// `only_owner()` has regressed to a presence-only check.
#[wasm_bindgen_test]
fn test_only_owner_rejects_zero_value_auth_transfer() -> Result<()> {
    clear();
    let deploy_block = deploy(880_000)?;
    let auth_token = auth_token_of(OWNED_TOKEN);

    // Build up the stash that would fund the forged `authenticate` call.
    let mut auth_outpoint = find_outpoint_holding(&deploy_block, auth_token, 1);
    for i in 1..=3u32 {
        auth_outpoint = owner_mint(880_000 + i, auth_token, auth_outpoint, 100)?;
    }
    let attacker = find_deployed(alkanes_std_test_build::get_bytes());
    let stash = alkane_inventory(OWNED_TOKEN, auth_token);
    assert!(stash > 0, "precondition: owned-token must hold an auth stash");

    let supply_before = alkane_inventory(OWNED_TOKEN, OWNED_TOKEN);

    // Attacker sends NO alkanes at all into the message; it fabricates the
    // zero-value auth transfer inside the extcall parcel.
    let block_height = 880_100;
    let mut block = protorune::test_helpers::create_block_with_coinbase_tx(block_height);
    let attack_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: attacker,
            inputs: vec![
                EXT_CALL_WITH_TRANSFER,
                OWNED_TOKEN.block,
                OWNED_TOKEN.tx,
                auth_token.block,
                auth_token.tx,
                0, /* transfer_value: the whole point */
                2, /* len of the forwarded inputs vec */
                MINT_OPCODE,
                1_000_000,
            ],
        }],
        OutPoint::default(),
        false,
    );
    block.txdata.push(attack_tx.clone());
    index_block(&block, block_height)?;

    let attack_outpoint = OutPoint {
        txid: attack_tx.compute_txid(),
        vout: 0,
    };
    // The message protostone is virtual vout 3 (2 real outputs + 1).
    alkane_helpers::assert_revert_context(
        &OutPoint {
            txid: attack_tx.compute_txid(),
            vout: 3,
        },
        "Auth token is not in incoming alkanes",
    )?;
    // No freshly minted OWNED reached the attacker.
    assert_eq!(
        outpoint_sheet(&attack_outpoint).get_cached(&OWNED_TOKEN.into()),
        0,
        "zero-value auth transfer must not authorize a mint"
    );
    assert_eq!(
        alkane_inventory(OWNED_TOKEN, OWNED_TOKEN),
        supply_before,
        "guarded contract state must be unchanged after a rejected call"
    );

    Ok(())
}

/// The top-level path was never exploitable: `RuneTransfer::from_balance_sheet`
/// filters out zero balances before they become `incoming_alkanes`, so an edict
/// can never deliver a `value == 0` entry from a Bitcoin transaction.
#[wasm_bindgen_test]
fn test_zero_value_edict_cannot_authorize() -> Result<()> {
    clear();
    deploy(880_000)?;

    let block_height = 880_100;
    let mut block = protorune::test_helpers::create_block_with_coinbase_tx(block_height);
    // Attacker spends an outpoint holding nothing and edicts the auth token.
    let attack_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in_with_edicts(
        Witness::new(),
        vec![
            alkane_helpers::CellpackOrEdict::Edict(vec![ProtostoneEdict {
                id: auth_token_of(OWNED_TOKEN).into(),
                amount: 0,
                output: 0,
            }]),
            alkane_helpers::CellpackOrEdict::Cellpack(Cellpack {
                target: OWNED_TOKEN,
                inputs: vec![MINT_OPCODE, 1_000_000],
            }),
        ],
        OutPoint::default(),
        false,
    );
    block.txdata.push(attack_tx.clone());
    index_block(&block, block_height)?;

    let attack_outpoint = OutPoint {
        txid: attack_tx.compute_txid(),
        vout: 0,
    };
    assert_eq!(
        outpoint_sheet(&attack_outpoint).get_cached(&OWNED_TOKEN.into()),
        0,
        "a zero-amount edict must not authorize a mint"
    );

    Ok(())
}
