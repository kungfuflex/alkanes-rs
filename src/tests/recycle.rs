//! Tests for the `8:dead` recycle bin (capture side + shared codec/format).
//!
//! Capture (`src/recycle.rs`, `IndexPointer`) and the `8:dead` claim WASM
//! (`StoragePointer`) build the `/recycle/<spk>` ledger key through the SAME
//! `KeyValuePointer` default `keyword`/`select` impls on an identically-wrapped
//! `Vec`, so the keys are byte-identical by construction. Here we lock the
//! capture-side key format + the shared ledger codec + the EOA gate so an
//! accidental change is caught at `cargo test` time. (A full end-to-end claim
//! test that runs the real WASM is tracked as a follow-up; it additionally
//! exercises the parity through an actual `8:dead:3` execution.)

use crate::recycle::{decode_ledger, encode_ledger};
use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::ProtoruneRuneId;

const RECYCLE_LEDGER_PREFIX: &str = "/recycle/";

fn p2tr(byte: u8) -> ScriptBuf {
    ScriptBuf::new_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(
        bitcoin::XOnlyPublicKey::from_slice(&[byte; 32]).unwrap(),
    ))
}

/// The capture-side ledger key is deterministic, embeds the script_pubkey, and
/// distinguishes distinct recipients. This locks the key format the claim WASM
/// must (and structurally does) mirror.
#[test]
fn ledger_key_is_deterministic_and_partitioned() {
    let a = p2tr(2).to_bytes();
    let b = p2tr(9).to_bytes();
    let key = |spk: &Vec<u8>| -> Vec<u8> {
        IndexPointer::from_keyword(RECYCLE_LEDGER_PREFIX)
            .select(spk)
            .unwrap()
            .as_ref()
            .clone()
    };
    assert_eq!(key(&a), key(&a), "same spk → same key");
    assert_ne!(key(&a), key(&b), "distinct recipients → distinct keys (partition)");
    // key must actually depend on the spk bytes (not collapse to the prefix)
    assert_ne!(
        key(&a),
        IndexPointer::from_keyword(RECYCLE_LEDGER_PREFIX)
            .unwrap()
            .as_ref()
            .clone()
    );
}

/// The ledger codec is shared verbatim between capture and the claim WASM.
/// Round-trip and confirm the 48-byte (block,tx,value) LE triple framing.
#[test]
fn ledger_codec_roundtrip_and_framing() {
    let entries = vec![
        (ProtoruneRuneId { block: 2, tx: 0x80424 }, 148_999_796u128),
        (ProtoruneRuneId { block: 2, tx: 0 }, u128::MAX),
        (ProtoruneRuneId { block: 8, tx: 0xdead }, 1),
    ];
    let blob = encode_ledger(&entries);
    assert_eq!(blob.len(), entries.len() * 48, "48 bytes per entry");
    assert_eq!(decode_ledger(&blob), entries);
    // trailing bytes shorter than a full triple are ignored, never panic
    let mut truncated = blob.clone();
    truncated.extend_from_slice(&[0xab; 10]);
    assert_eq!(decode_ledger(&truncated), entries);
    // a zeroed (claimed) entry decodes to nothing
    assert!(decode_ledger(&[]).is_empty());
}

/// EOA gate: capture credits / claim releases only key-path outputs (mirrors
/// `is_eoa` on both sides).
#[test]
fn is_eoa_classification() {
    fn is_eoa(spk: &ScriptBuf) -> bool {
        spk.is_p2tr() || spk.is_p2wpkh() || spk.is_p2pkh()
    }
    let p2wpkh = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([3u8; 20]));
    let p2pkh = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_byte_array([4u8; 20]));
    let p2wsh = ScriptBuf::new_p2wsh(&bitcoin::WScriptHash::from_byte_array([5u8; 32]));
    let op_return = ScriptBuf::new_op_return([0u8; 4]);

    assert!(is_eoa(&p2tr(2)));
    assert!(is_eoa(&p2wpkh));
    assert!(is_eoa(&p2pkh));
    assert!(!is_eoa(&p2wsh), "script-path (contract-like) is not EOA");
    assert!(!is_eoa(&op_return), "OP_RETURN is never a recycle recipient");
}

/// End-to-end: seed an alkane at an outpoint, strand it with a protostone-less
/// spend (native capture sweeps it into 8:dead), then claim it back via the
/// 8:dead wasm (opcode 3). Exercises capture↔claim key-parity, the EOA caller
/// gate, and the anti-mint inventory accounting through a real VM execution.
#[cfg(test)]
mod e2e {
    use crate::index_block;
    use crate::recycle::RECYCLE_ALKANE_ID;
    use crate::message::AlkaneMessageContext;
    use crate::tests::helpers as h;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use bitcoin::hashes::Hash;
    use bitcoin::{
        Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    };
    use metashrew_core::index_pointer::IndexPointer;
    use metashrew_support::index_pointer::KeyValuePointer;
    use protorune::balance_sheet::{load_sheet, PersistentRecord};
    use protorune::message::MessageContext;
    use protorune::tables::{RuneTable, OUTPOINT_TO_OUTPUT};
    use prost::Message as _;
    use protorune_support::balance_sheet::{
        BalanceSheet, BalanceSheetOperations, ProtoruneRuneId,
    };
    use protorune_support::utils::consensus_encode;
    use std::sync::Arc;


    /// Pins the post-audit fork above the test blocks; restores it on drop.
    struct PreForkGuard;
    impl PreForkGuard {
        fn new() -> Self {
            protorune::set_post_audit_fork_height_override(Some(1_000));
            PreForkGuard
        }
    }
    impl Drop for PreForkGuard {
        fn drop(&mut self) {
            protorune::set_post_audit_fork_height_override(None);
        }
    }

    /// Register `op`'s scriptPubKey the way `index_outpoints` does for every
    /// output of every indexed transaction. Capture keys the recycle ledger off
    /// the PREVOUT OWNER, so a synthetic seed outpoint has to declare its owner
    /// or it looks like a pre-indexing outpoint with no claimant.
    fn seed_prevout_script(op: &OutPoint, spk: &ScriptBuf) -> Result<()> {
        OUTPOINT_TO_OUTPUT.select(&consensus_encode(op)?).set(Arc::new(
            (protorune_support::proto::protorune::Output {
                script: spk.clone().into_bytes(),
                value: 546,
            })
            .encode_to_vec(),
        ));
        Ok(())
    }

    fn eoa_recovery_spk() -> ScriptBuf {
        ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([7u8; 20]))
    }

    /// `/alkanes/<what>/balances/<8:dead>` — what the claim WASM debits.
    fn recycle_inventory_balance(what: &AlkaneId) -> u128 {
        let what_bytes: Vec<u8> = what.clone().into();
        let who_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        IndexPointer::from_keyword("/alkanes/")
            .select(&what_bytes)
            .keyword("/balances/")
            .select(&who_bytes)
            .get_value::<u128>()
    }

    /// Decode the `8:dead` ledger at `/alkanes/<8:dead>/storage/recycle/<spk>`.
    fn recycle_ledger(spk: &[u8]) -> Vec<(ProtoruneRuneId, u128)> {
        let inner: Vec<u8> = IndexPointer::from_keyword("/recycle/")
            .select(&spk.to_vec())
            .unwrap()
            .as_ref()
            .clone();
        let id_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        let raw = IndexPointer::from_keyword("/alkanes/")
            .select(&id_bytes)
            .keyword("/storage/")
            .select(&inner)
            .get();
        crate::recycle::decode_ledger(raw.as_ref())
    }

    #[test]
    fn recycle_capture_then_claim_roundtrip() -> Result<()> {
        h::clear();
        // Index below the post-audit fork: from the fork on, protorune settles
        // a protostone-less spend to default_output itself and capture sees
        // nothing. The recycle bin recovers the pre-fork strandings.
        let _fork = PreForkGuard::new();
        let tag = AlkaneMessageContext::protocol_tag();
        let table = RuneTable::for_protocol(tag);
        let stranded = AlkaneId { block: 2, tx: 1234 };
        let stranded_rune = ProtoruneRuneId { block: 2, tx: 1234 };
        let amount: u128 = 500;
        let recovery = eoa_recovery_spk();

        // 1) seed an outpoint that carries the alkane
        let seed_op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([9u8; 32]),
            vout: 0,
        };
        let mut seed_sheet = BalanceSheet::<IndexPointer>::default();
        seed_sheet.increase(&stranded_rune, amount)?;
        seed_sheet.save(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?), false);
        // The victim owned the outpoint being spent — that is who capture credits.
        seed_prevout_script(&seed_op, &recovery)?;

        // 2) strand it: a tx with NO OP_RETURN spending seed_op, paying recovery EOA
        let strand_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: seed_op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: recovery.clone(),
            }],
        };
        let block1 = protorune::test_helpers::create_block_with_txs(vec![strand_tx]);
        index_block(&block1, 1)?;

        // 3) capture assertions
        assert_eq!(
            recycle_inventory_balance(&stranded),
            amount,
            "8:dead inventory should hold the stranded alkane"
        );
        assert_eq!(
            recycle_ledger(recovery.as_bytes()),
            vec![(stranded_rune.clone(), amount)],
            "ledger[recovery] should record the stranded balance"
        );
        // capture clears the spent input (clear_balances).
        assert_eq!(
            load_sheet(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?))
                .get_cached(&stranded_rune),
            0,
            "stranded input balance should be cleared after capture"
        );

        // 4) claim via 8:dead opcode 3 — output[0] is the recovery EOA
        let claim_cellpack = Cellpack {
            target: RECYCLE_ALKANE_ID,
            inputs: vec![3],
        };
        let protostone = protorune_support::protostone::Protostone {
            message: claim_cellpack.encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: tag,
        };
        let dummy_in = TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([1u8; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };
        let claim_tx = h::create_protostone_tx_with_inputs(
            vec![dummy_in],
            vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: recovery.clone(),
            }],
            protostone,
        );
        let block2 = protorune::test_helpers::create_block_with_txs(vec![claim_tx]);
        index_block(&block2, 2)?;

        // 5) claim assertions: recovery output (vout 0) holds the alkane; ledger zeroed.
        let claim_outpoint = OutPoint {
            txid: block2.txdata[0].compute_txid(),
            vout: 0,
        };
        let claimed = load_sheet(
            &table.OUTPOINT_TO_RUNES.select(&consensus_encode(&claim_outpoint)?),
        );
        assert_eq!(
            claimed.get_cached(&stranded_rune),
            amount,
            "claim should release the stranded alkane to the recovery output"
        );
        assert!(
            recycle_ledger(recovery.as_bytes()).is_empty(),
            "ledger entry should be zeroed after claim (no replay)"
        );
        let _ = Arc::new(());
        Ok(())
    }

    /// SECURITY: an attacker must NOT be able to claim a *victim's* stranded
    /// balances by simply placing the victim's spk at vout 0 (to satisfy the
    /// `recipient_script()` lookup against `/recycle/<victim_spk>`) while
    /// routing the released alkanes to a different output via the protostone
    /// `pointer`. The claim emits its alkanes into `response.alkanes`, which
    /// `reconcile` deposits at `parcel.pointer` — independent of the ledger
    /// key. This PoC exercises that gap end-to-end and asserts the safe
    /// outcome (alkanes returned to the victim's vout 0, attacker's vout 1
    /// gets nothing). If this test FAILS — i.e. the attacker's output ends up
    /// holding `amount` and vout 0 is empty — the recycle bin allows theft.
    #[test]
    fn recycle_claim_cannot_be_redirected_via_protostone_pointer() -> Result<()> {
        h::clear();
        // Index below the post-audit fork: from the fork on, protorune settles
        // a protostone-less spend to default_output itself and capture sees
        // nothing. The recycle bin recovers the pre-fork strandings.
        let _fork = PreForkGuard::new();
        let tag = AlkaneMessageContext::protocol_tag();
        let table = RuneTable::for_protocol(tag);
        let stranded = AlkaneId { block: 2, tx: 4321 };
        let stranded_rune = ProtoruneRuneId { block: 2, tx: 4321 };
        let amount: u128 = 777;
        let victim = eoa_recovery_spk();
        // distinct EOA, distinct key material — must not collide with `victim`.
        let attacker: ScriptBuf =
            ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([0xAAu8; 20]));
        assert_ne!(victim, attacker, "victim/attacker spks must differ");

        // 1) seed an outpoint that carries the alkane
        let seed_op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([0x42u8; 32]),
            vout: 0,
        };
        let mut seed_sheet = BalanceSheet::<IndexPointer>::default();
        seed_sheet.increase(&stranded_rune, amount)?;
        seed_sheet.save(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?), false);
        // The victim owned the outpoint being spent — that is who capture credits.
        seed_prevout_script(&seed_op, &victim)?;

        // 2) strand it: bare-BTC spend paying victim EOA → capture credits 8:dead
        //    inventory and writes /recycle/<victim_spk>.
        let strand_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: seed_op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: victim.clone(),
            }],
        };
        let block1 = protorune::test_helpers::create_block_with_txs(vec![strand_tx]);
        index_block(&block1, 1)?;

        // Sanity: the ledger now owes the victim.
        assert_eq!(
            recycle_inventory_balance(&stranded),
            amount,
            "8:dead inventory should hold the stranded alkane after capture"
        );
        assert_eq!(
            recycle_ledger(victim.as_bytes()),
            vec![(stranded_rune.clone(), amount)],
            "victim ledger should record the stranded balance"
        );

        // 3) ATTACK: attacker builds a claim tx where vout 0 is the *victim's*
        //    spk (so `recipient_script()` reads /recycle/<victim_spk>) and vout
        //    1 is the attacker's spk. The protostone pointer is set to 1 so
        //    `reconcile` deposits the released alkanes at the attacker's output.
        let claim_cellpack = Cellpack {
            target: RECYCLE_ALKANE_ID,
            inputs: vec![3],
        };
        let protostone = protorune_support::protostone::Protostone {
            message: claim_cellpack.encipher(),
            pointer: Some(1),   // ← attempted theft: payout to attacker's vout
            refund: Some(1),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: tag,
        };
        // Use a fresh dummy input the attacker controls — distinct txid from
        // any prior fixture so capture in this block can't write under it.
        let dummy_in = TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([0xCCu8; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };
        let claim_tx = h::create_protostone_tx_with_inputs(
            vec![dummy_in],
            vec![
                TxOut {
                    value: Amount::from_sat(546),
                    script_pubkey: victim.clone(),    // vout 0 = victim's spk (ledger key)
                },
                TxOut {
                    value: Amount::from_sat(546),
                    script_pubkey: attacker.clone(),  // vout 1 = attacker — payout target
                },
            ],
            protostone,
        );
        let block2 = protorune::test_helpers::create_block_with_txs(vec![claim_tx]);
        index_block(&block2, 2)?;

        let claim_txid = block2.txdata[0].compute_txid();
        let victim_out = OutPoint { txid: claim_txid, vout: 0 };
        let attacker_out = OutPoint { txid: claim_txid, vout: 1 };
        let victim_sheet = load_sheet(
            &table.OUTPOINT_TO_RUNES.select(&consensus_encode(&victim_out)?),
        );
        let attacker_sheet = load_sheet(
            &table.OUTPOINT_TO_RUNES.select(&consensus_encode(&attacker_out)?),
        );

        // SECURITY EXPECTATION: the attacker's output gets nothing. The only
        // safe outcomes are (a) the claim reverted (both outputs empty, ledger
        // intact) or (b) the claim succeeded and routed payout to the victim.
        // The unsafe outcome — attacker_sheet[stranded_rune] == amount — means
        // burned alkanes can be stolen by anyone who can name the victim's spk.
        assert_eq!(
            attacker_sheet.get_cached(&stranded_rune),
            0,
            "VULN: attacker's vout received victim's burned alkanes — \
             recycle claim payout is steerable via protostone pointer"
        );
        // If the claim was allowed to succeed under this construction, the
        // payout must have gone to the victim and the ledger must be cleared.
        // If the claim was instead rejected, the ledger should still owe the
        // victim (so the victim can retry safely).
        let ledger_after = recycle_ledger(victim.as_bytes());
        if !ledger_after.is_empty() {
            assert_eq!(
                ledger_after,
                vec![(stranded_rune.clone(), amount)],
                "rejected claim must leave the victim's ledger intact"
            );
            assert_eq!(
                victim_sheet.get_cached(&stranded_rune),
                0,
                "rejected claim must not have moved alkanes anywhere"
            );
        } else {
            assert_eq!(
                victim_sheet.get_cached(&stranded_rune),
                amount,
                "successful claim must have paid the victim's vout, not the attacker's"
            );
        }
        let _ = Arc::new(());
        Ok(())
    }

    /// Sets `RECYCLE_CLAIM_HEIGHT` for the test; restores it on drop.
    struct ClaimGateGuard;
    impl ClaimGateGuard {
        fn new(height: u64) -> Self {
            crate::recycle::set_recycle_claim_height_override(Some(height));
            ClaimGateGuard
        }
    }
    impl Drop for ClaimGateGuard {
        fn drop(&mut self) {
            crate::recycle::set_recycle_claim_height_override(None);
        }
    }

    /// A tx paying `spk` at vout 0 that claims from 8:dead (opcode 3, pointer 0).
    fn recycle_claim_tx(spk: &ScriptBuf, funding: u8) -> Transaction {
        let protostone = protorune_support::protostone::Protostone {
            message: Cellpack {
                target: RECYCLE_ALKANE_ID,
                inputs: vec![3],
            }
            .encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: AlkaneMessageContext::protocol_tag(),
        };
        h::create_protostone_tx_with_inputs(
            vec![TxIn {
                previous_output: OutPoint {
                    txid: bitcoin::Txid::from_byte_array([funding; 32]),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: spk.clone(),
            }],
            protostone,
        )
    }

    /// Capture runs from genesis, but a claim below RECYCLE_CLAIM_HEIGHT is not
    /// processed: nothing is released and the ledger and inventory stay intact.
    /// The same claim succeeds once the gate is reached.
    #[test]
    fn recycle_claim_waits_for_activation_height() -> Result<()> {
        h::clear();
        let _fork = PreForkGuard::new();
        let gate: u32 = 5;
        let _gate = ClaimGateGuard::new(gate as u64);
        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        let stranded = AlkaneId { block: 2, tx: 1234 };
        let stranded_rune = ProtoruneRuneId { block: 2, tx: 1234 };
        let amount: u128 = 500;
        let recovery = eoa_recovery_spk();

        // Seed and strand at block 1, below the gate: capture still runs.
        let seed_op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([9u8; 32]),
            vout: 0,
        };
        let mut seed_sheet = BalanceSheet::<IndexPointer>::default();
        seed_sheet.increase(&stranded_rune, amount)?;
        seed_sheet.save(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?), false);
        // The victim owned the outpoint being spent — that is who capture credits.
        seed_prevout_script(&seed_op, &recovery)?;
        let strand_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: seed_op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: recovery.clone(),
            }],
        };
        index_block(&protorune::test_helpers::create_block_with_txs(vec![strand_tx]), 1)?;
        assert_eq!(recycle_inventory_balance(&stranded), amount);
        assert_eq!(
            recycle_ledger(recovery.as_bytes()),
            vec![(stranded_rune.clone(), amount)]
        );

        let claimed_at = |block: &bitcoin::Block| -> Result<u128> {
            let out = OutPoint {
                txid: block.txdata[0].compute_txid(),
                vout: 0,
            };
            Ok(load_sheet(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&out)?))
                .get_cached(&stranded_rune))
        };

        // Claim one block below the gate: not processed.
        let early = protorune::test_helpers::create_block_with_txs(vec![recycle_claim_tx(
            &recovery, 1,
        )]);
        index_block(&early, gate - 1)?;
        assert_eq!(claimed_at(&early)?, 0, "pre-gate claim must release nothing");
        assert_eq!(
            recycle_ledger(recovery.as_bytes()),
            vec![(stranded_rune.clone(), amount)],
            "pre-gate claim must leave the ledger intact"
        );
        assert_eq!(
            recycle_inventory_balance(&stranded),
            amount,
            "pre-gate claim must leave the inventory intact"
        );

        // Claim at the gate: released to the recovery output.
        let live = protorune::test_helpers::create_block_with_txs(vec![recycle_claim_tx(
            &recovery, 2,
        )]);
        index_block(&live, gate)?;
        assert_eq!(claimed_at(&live)?, amount, "claim at the gate must release");
        assert!(recycle_ledger(recovery.as_bytes()).is_empty());
        assert_eq!(recycle_inventory_balance(&stranded), 0);
        Ok(())
    }

    /// A distinct EOA scriptPubKey per `byte`. p2wpkh rather than p2tr because
    /// the hash is arbitrary bytes — no curve-point validity to satisfy — and
    /// `is_eoa` accepts it just the same.
    fn eoa(byte: u8) -> ScriptBuf {
        ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([byte; 20]))
    }

    /// Strand `amount` of `asset` at `op`, owned by `owner`, and capture a
    /// protostone-less spend of it that pays `pays`.
    fn strand_and_capture(
        op: OutPoint,
        owner: &ScriptBuf,
        pays: &ScriptBuf,
        asset: &ProtoruneRuneId,
        amount: u128,
    ) -> Result<()> {
        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        seed_prevout_script(&op, owner)?;
        let mut sheet = BalanceSheet::<IndexPointer>::default();
        sheet.increase(asset, amount)?;
        sheet.save(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&op)?), false);
        let spend = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: pays.clone(),
            }],
        };
        crate::recycle::capture_block(
            &protorune::test_helpers::create_block_with_txs(vec![spend]),
            AlkaneMessageContext::protocol_tag(),
        )
    }

    /// The mis-credit vector: a transaction that spends victim `V`'s stranded
    /// outpoint while paying output 0 to `A` must NOT put V's alkanes in A's
    /// claimable ledger. (The claim side then binds the release to the payout
    /// output, so only V paying V can take them out.)
    #[test]
    fn capture_credits_prevout_owner_not_output_zero() -> Result<()> {
        h::clear();
        let victim = eoa(7);
        let attacker = eoa(200);
        let table = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag());
        let op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([0x51u8; 32]),
            vout: 0,
        };
        let asset = ProtoruneRuneId { block: 2, tx: 81974 };
        strand_and_capture(op, &victim, &attacker, &asset, 364)?;

        assert_eq!(
            crate::recycle::recycled_balances(victim.as_bytes()),
            vec![(asset.clone(), 364u128)],
            "the prevout owner must be credited the stranded alkanes"
        );
        assert!(
            crate::recycle::recycled_balances(attacker.as_bytes()).is_empty(),
            "output 0 of the spending tx must NOT be credited"
        );
        assert_eq!(
            load_sheet(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&op)?))
                .get_cached(&asset),
            0,
            "stranded balance must be cleared"
        );
        Ok(())
    }

    /// `getrecycled` reports exactly what is claimable, clamped to `8:dead`'s
    /// held inventory, and is empty for an address with no ledger.
    #[test]
    fn getrecycled_view_reports_claimable_balances() -> Result<()> {
        h::clear();
        let owner = eoa(11);
        let stranger = eoa(12);
        let op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([0x52u8; 32]),
            vout: 3,
        };
        let asset = ProtoruneRuneId { block: 2, tx: 77623 };
        strand_and_capture(op, &owner, &owner, &asset, 1_813)?;

        let ask = |spk: &ScriptBuf| -> Result<Vec<(AlkaneId, u128)>> {
            let req = alkanes_support::proto::alkanes::AlkaneRecycledRequest {
                script: spk.clone().into_bytes(),
            };
            let resp = crate::view::getrecycled(&req.encode_to_vec())?;
            Ok(resp
                .alkanes
                .into_iter()
                .map(|t| {
                    let id: AlkaneId = t.id.clone().unwrap().into();
                    let v: u128 = t.value.clone().unwrap().into();
                    (id, v)
                })
                .collect())
        };

        assert_eq!(
            ask(&owner)?,
            vec![(AlkaneId { block: 2, tx: 77623 }, 1_813u128)],
            "getrecycled must report the owner's claimable balance sheet"
        );
        assert!(
            ask(&stranger)?.is_empty(),
            "getrecycled must be empty for an address with no ledger"
        );

        // Clamped to held inventory: drain 8:dead below the ledger amount.
        let what_bytes: Vec<u8> = AlkaneId { block: 2, tx: 77623 }.into();
        let who_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        IndexPointer::from_keyword("/alkanes/")
            .select(&what_bytes)
            .keyword("/balances/")
            .select(&who_bytes)
            .set_value::<u128>(1_000);
        assert_eq!(
            ask(&owner)?,
            vec![(AlkaneId { block: 2, tx: 77623 }, 1_000u128)],
            "getrecycled must clamp to 8:dead's held inventory"
        );
        Ok(())
    }
}
