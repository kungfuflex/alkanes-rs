use crate::balance_sheet::OutgoingRunes;
use crate::{
    message::{MessageContext, MessageContextParcel},
    protoburn::{Protoburn, Protoburns},
};
use anyhow::{anyhow, Result};
use bitcoin::{Block, Transaction, Txid};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use ordinals::Runestone;
use protorune_support::{
    balance_sheet::BalanceSheet,
    protostone::{split_bytes, Protostone},
    rune_transfer::{refund_to_refund_pointer, RuneTransfer},
    utils::encode_varint_list,
};
use std::collections::{BTreeMap, BTreeSet};

use metashrew_core::{println, stdio::stdout};
use std::fmt::Write;

static mut PROTOCOLS: Option<BTreeSet<u128>> = None;

#[allow(static_mut_refs)]
pub fn initialized_protocol_index() -> Result<()> {
    unsafe { PROTOCOLS = Some(BTreeSet::new()) }
    Ok(())
}

#[allow(static_mut_refs)]
pub fn add_to_indexable_protocols(protocol_tag: u128) -> Result<()> {
    unsafe {
        if let Some(set) = PROTOCOLS.as_mut() {
            set.insert(protocol_tag);
        }
    }
    Ok(())
}

pub trait MessageProcessor {
    ///
    /// Parameters:
    ///   atomic: Atomic pointer to hold changes to the index,
    ///           will only be committed upon success
    ///   transaction: The current transaction
    ///   txindex: The current transaction's index in the block
    ///   block: The current block
    ///   height: The current block height
    ///   _runestone_output_index: TODO: not used??
    ///   protomessage_vout: The vout of the current protomessage. These are "virtual"
    ///                 vouts, meaning they are greater than the number of real vouts
    ///                 and increase by 1 for each new protostone in the op_return.
    ///                 Protoburns and protostone edicts can target these vouts, so they
    ///                 will hold balances before the process message
    ///   balances_by_output: The running store of balances by each transaction output for
    ///                       the current transaction being handled.
    /// Return: true if success, false if failure and refunded to refund pointer
    fn process_message<T: MessageContext>(
        &self,
        atomic: &mut AtomicPointer,
        transaction: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        _runestone_output_index: u32,
        protomessage_vout: u32,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        num_protostones: usize,
    ) -> Result<bool>;
}
impl MessageProcessor for Protostone {
    fn process_message<T: MessageContext>(
        &self,
        atomic: &mut AtomicPointer,
        transaction: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        _runestone_output_index: u32,
        protomessage_vout: u32,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        num_protostones: usize,
    ) -> Result<bool> {
        // Validate output indexes and protomessage_vout
        let num_outputs = transaction.output.len();

        // A malformed protostone header (absent or out-of-range pointer /
        // refund_pointer) used to return `Err` from all three checks below.
        // `Err` is NOT "this message failed" — it propagates out of
        // `index_protostones` and voids the WHOLE transaction's protorune
        // processing, including protostones that already succeeded, leaving the
        // spent inputs' alkanes stranded with no refund. `Invalid output
        // pointer` in particular is reachable by any malformed wallet, not just
        // an exotic batch.
        //
        // At/after the fork these become an ordinary failed message: `Ok(false)`
        // plus an explicit refund, exactly like a reverting call. (`Ok(false)`
        // alone does NOT refund — every failure path in this function pairs it
        // with `refund_to_refund_pointer`.)
        //
        // Gated on the same height as the `max_virtual_vout` removal, because it
        // changes how historical blocks index. Below it, the legacy `Err` stands.
        let bound = (num_outputs + num_protostones) as u32;
        let header_is_malformed = match (self.pointer, self.refund) {
            (Some(p), Some(r)) => p > bound || r > bound,
            _ => true,
        };
        if header_is_malformed && height >= T::max_virtual_vout_removal_height() {
            // Prefer the declared refund_pointer when it names a REAL output —
            // that is the author's stated intent and is spendable. A shadow vout
            // is rejected here even though it is in `bound`: it addresses another
            // protostone, which may itself fail, and we must not re-strand.
            // Otherwise fall back to `default_output` (first non-OP_RETURN), the
            // protocol's own convention for unallocated runes.
            //
            // Degenerate case: a transaction whose outputs are ALL OP_RETURN has
            // nowhere spendable to send this, and `default_output` yields 0 — the
            // refund burns. That is unavoidable and matches existing protorune
            // semantics for unallocated balances.
            let safe_refund = self
                .refund
                .filter(|r| (*r as usize) < num_outputs)
                .unwrap_or_else(|| crate::default_output(transaction));
            refund_to_refund_pointer(balances_by_output, protomessage_vout, safe_refund)?;
            return Ok(false);
        }

        let pointer = self.pointer.ok_or_else(|| anyhow!("Missing pointer"))?;
        let refund_pointer = self
            .refund
            .ok_or_else(|| anyhow!("Missing refund pointer"))?;

        // Ensure pointers are valid transaction outputs
        if pointer > bound || refund_pointer > bound {
            return Err(anyhow::anyhow!("Invalid output pointer"));
        }

        // Log the Bitcoin address that can spend the output pointed to by the "pointer" field
        if pointer < num_outputs as u32 {
            if let Ok(address) = protorune_support::network::to_address_str(
                &transaction.output[pointer as usize].script_pubkey,
            ) {
                println!(
                    "Protostone pointer ({}) points to Bitcoin address: {}",
                    pointer, address
                );
            }
        }

        // Log the Bitcoin address that can spend the output pointed to by the "refund_pointer" field
        if refund_pointer < num_outputs as u32 {
            if let Ok(address) = protorune_support::network::to_address_str(
                &transaction.output[refund_pointer as usize].script_pubkey,
            ) {
                println!(
                    "Protostone refund_pointer ({}) points to Bitcoin address: {}",
                    refund_pointer, address
                );
            }
        }

        // Legacy protostone-count cap, retired at
        // `T::max_virtual_vout_removal_height()`.
        //
        // `num_outputs + 100` is an artifact of the era when OP_RETURN was
        // limited to 80 bytes, which made >100 protostones unrepresentable
        // anyway. It guards nothing: `protomessage_vout` is a u32 key into a
        // `BTreeMap`, not an array index, so there is no overflow to prevent,
        // and unbounded VM work is already bounded by fuel. It also contradicts
        // the pointer bound above, which admits pointers up to
        // `num_outputs + num_protostones`.
        //
        // It cannot simply be deleted: it has been consensus since 2025-02-26
        // and historical balances depend on it, so removal is height-gated.
        // NOTE the failure mode below the fork: this returns `Err`, not
        // `Ok(false)`, so it does NOT take the refund path — it propagates out
        // of `index_protostones` and voids the whole transaction's protorune
        // processing. Assets on the spent inputs are left stranded rather than
        // refunded, and are recovered from the `8:dead` recycle bin
        // (`alkanes::recycle::capture_block`), which credits them to the
        // address that owned the prevout.
        if height < T::max_virtual_vout_removal_height() {
            let max_virtual_vout = num_outputs + 100;
            if protomessage_vout >= max_virtual_vout as u32 {
                return Err(anyhow::anyhow!("Protomessage vout exceeds maximum allowed"));
            }
        }
        let initial_sheet = balances_by_output
            .get(&protomessage_vout)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::default());

        // Snapshot the NON-transactional in-memory balance map so a failed message
        // rolls back in LOCKSTEP with `atomic`. This binds the two stores: on
        // revert, `atomic.rollback()` unwinds the KV side and restoring this
        // snapshot unwinds the in-memory side. No partial in-memory mutation (e.g.
        // a `reconcile` that removed the incoming vout before overflowing on the
        // pointer) can survive a revert, so the whole "two stores, one rollback"
        // bug class cannot exist here regardless of the internals of `reconcile`
        // or any future in-memory mutation added to the message path.
        let balances_snapshot = balances_by_output.clone();

        // Create a nested atomic transaction for the entire message processing
        atomic.checkpoint();

        let parcel = MessageContextParcel {
            atomic: atomic.derive(&IndexPointer::default()),
            runes: RuneTransfer::from_balance_sheet(initial_sheet.clone()),
            transaction: transaction.clone(),
            block: block.clone(),
            height,
            vout: protomessage_vout,
            pointer,
            refund_pointer,
            calldata: self.message.iter().flat_map(|v| v.to_be_bytes()).collect(),
            txindex,
            runtime_balances: Box::new(
                balances_by_output
                    .get(&u32::MAX)
                    .map(|v| v.clone())
                    .unwrap_or_else(|| BalanceSheet::default()),
            ),
            sheets: Box::new(BalanceSheet::default()),
        };

        match T::handle(&parcel) {
            Ok(values) => {
                match values.reconcile(atomic, balances_by_output, protomessage_vout, pointer) {
                    Ok(_) => {
                        atomic.commit();
                        Ok(true)
                    }
                    Err(e) => {
                        println!("Got error inside reconcile! {:?} \n\n", e);
                        println!("Refunding to refund_pointer: {}", refund_pointer);

                        // Log the Bitcoin address again to make it clear this is the refund address being used
                        if refund_pointer < num_outputs as u32 {
                            if let Ok(address) = protorune_support::network::to_address_str(
                                &transaction.output[refund_pointer as usize].script_pubkey,
                            ) {
                                println!("RECONCILE ERROR REFUND: Protostone refund_pointer ({}) points to Bitcoin address: {}", refund_pointer, address);
                            }
                        }

                        // Restore the in-memory map to its pre-message state, roll
                        // back the KV side, THEN refund from the clean snapshot.
                        // Rolling back BEFORE the (fallible) refund also means a
                        // refund-side overflow can no longer leave this message's
                        // checkpoint dangling on the stack.
                        *balances_by_output = balances_snapshot;
                        atomic.rollback();
                        refund_to_refund_pointer(
                            balances_by_output,
                            protomessage_vout,
                            refund_pointer,
                        )?;
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                println!("Alkanes message reverted with error: {:?}", e);
                println!("Refunding to refund_pointer: {}", refund_pointer);

                // Log the Bitcoin address again to make it clear this is the refund address being used
                if refund_pointer < num_outputs as u32 {
                    if let Ok(address) = protorune_support::network::to_address_str(
                        &transaction.output[refund_pointer as usize].script_pubkey,
                    ) {
                        println!(
                            "REFUND: Protostone refund_pointer ({}) points to Bitcoin address: {}",
                            refund_pointer, address
                        );
                    }
                }

                // Restore the in-memory map to its pre-message state, roll back the
                // KV side, THEN refund from the clean snapshot (see reconcile-Err
                // branch above for the ordering rationale).
                *balances_by_output = balances_snapshot;
                atomic.rollback();
                refund_to_refund_pointer(balances_by_output, protomessage_vout, refund_pointer)?;

                Ok(false)
            }
        }
    }
}

pub trait Protostones {
    fn burns(&self) -> Result<Vec<Protoburn>>;
    fn process_burns(
        &self,
        atomic: &mut AtomicPointer,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()>;
    fn encipher(&self) -> Result<Vec<u128>>;
}

impl Protostones for Vec<Protostone> {
    fn encipher(&self) -> Result<Vec<u128>> {
        let mut values = Vec::<u128>::new();
        for stone in self {
            values.push(stone.protocol_tag);
            let varints = stone.to_integers()?;
            values.push(varints.len() as u128);
            values.extend(&varints);
        }
        Ok(split_bytes(&encode_varint_list(&values)))
    }
    fn burns(&self) -> Result<Vec<Protoburn>> {
        Ok(self
            .into_iter()
            .filter(|stone| stone.burn.is_some())
            .map(|stone| Protoburn {
                tag: stone.burn.map(|v| v as u128),
                pointer: stone.pointer,
                from: stone.from.map(|v| vec![v]),
            })
            .collect())
    }
    fn process_burns(
        &self,
        atomic: &mut AtomicPointer,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()> {
        let mut burns = self.burns()?;
        burns.process(
            atomic,
            runestone.edicts.clone(),
            runestone_output_index,
            balances_by_output,
            proto_balances_by_output,
            default_output,
            txid,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use protorune_support::{balance_sheet::ProtoruneRuneId, protostone::ProtostoneEdict};

    /// Lets say we have a protostone defined as follows: vec<u128>![1 4 83 0 91 3]. This is a protostone with a protocol tag of 1, a length of 4, tag 83 (burn) is 0, tag 91 (pointer) is 3.
    /// Encoding:
    /// 1. Protocol step: Each u128 is LEB encoded. Each u128 becomes a vector of up to 16 bytes and is then concatenated together. LEB saves space by allowing smaller numbers to be one byte.
    ///         type: vec<u8>
    ///         [1 4 83 0 91 3]
    /// 2. Compression step: Combine the vec<u8> into a vec<u128> where we don't use the 16th byte. We should make the endianess such that the runes encodes is most efficient
    ///         type: vec<u128>. In this case, we can fit all our numbers into one u128.
    ///         this protostone becomes one u128 with bytes [1 4 83 0 91 3 0 0 0 0 0 0 0 0 0 0] or [0 0 0 0 0 0 0 0 0 0 3 91 0 83 4 1]
    ///         machine is little endian (wasm is little endian) = then we want to store it [1 4 83 0 91 3 0 0 0 0 0 0 0 0 0 0]
    ///         if machine was big endian = then we want to store it [0 0 0 0 0 0 0 0 0 0 3 91 0 83 4 1]
    ///
    ///         CONCLUSION:
    ///         since we are building to wasm, and wasm is little endian, we should store it with the data bytes at the lower memory address, so [1 4 83 0 91 3 0 0 0 0 0 0 0 0 0 0]
    /// 3. (Runes) LEB Encode each u128. The smaller the u128 the better.

    /// Assume runes already read the proto from tags.
    /// Decoding: proto is a vec<u128> (arbituary vector of u128 that we have to decode into a protostone) vec![u128([1 4 83 0 91 3 0 0 0 0 0 0 0 0 0 0])]
    /// 1. Undo the compression: convert each u128 into a vec<u8> and then concat to one array.
    ///         Important notes:
    ///          - We need to strip the 16th byte from each u128 to follow the spec
    ///          - [REMOVED] For the very last u128, we strip all postfix zeroes -- we don't want to do this because what if our input was like this?: vec![u128([1 4 91 3 83 0 0 0 0 0 0 0 0 0 0 0])]
    ///         input: vec![u128([1 4 83 0 91 3 0 0 0 0 0 0 0 0 0 0])]
    ///         output: vec<u8>![1 4 83 0 91 3 0 0 0 0 0 0 0 0 0]
    ///
    /// 2. Now we can LEB decode this vector of bytes into a vector of u128s. Note in this example, all numbers are less than 7 bits so their LEB representation is the same as the original u128.
    ///         input: vec<u8>![1 4 83 0 91 3 0 0 0 0 0 0 0 0 0]
    ///         output: vec<u128>![1 4 83 0 91 3 0 0 0 0 0 0 0 0 0]
    ///
    use super::*;

    #[test]
    fn test_protostone_encipher_burn() {
        let protostones = vec![Protostone {
            burn: Some(1u128),
            edicts: vec![],
            pointer: Some(3),
            refund: None,
            from: None,
            protocol_tag: 13, // must be 13 when protoburn
            message: vec![],
        }];

        let protostone_enciphered = protostones.encipher().unwrap();

        let protostone_decipered = Protostone::decipher(&protostone_enciphered).unwrap();

        assert_eq!(protostones, protostone_decipered);
    }

    #[test]
    fn test_protostone_encipher_edict() {
        let protostones = vec![Protostone {
            burn: Some(0u128),
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId {
                    block: 8400000,
                    tx: 1,
                },
                amount: 123456789,
                output: 2,
            }],
            pointer: Some(3),
            refund: None,
            from: None,
            protocol_tag: 1,
            message: vec![],
        }];

        let protostone_enciphered = protostones.encipher().unwrap();

        let protostone_decipered = Protostone::decipher(&protostone_enciphered).unwrap();

        assert_eq!(protostones, protostone_decipered);
    }

    #[test]
    fn test_protostone_encipher_multiple_u128() {
        let protostones = vec![Protostone {
            burn: None,
            edicts: vec![],
            pointer: Some(3),
            refund: None,
            from: None,
            protocol_tag: 1,
            message: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0], // what we pass in should be well defined by the subprotocol
        }];

        let protostone_enciphered = protostones.encipher().unwrap();

        let protostone_decipered = Protostone::decipher(&protostone_enciphered).unwrap();

        assert_eq!(protostones, protostone_decipered);
    }

    #[test]
    fn test_protostone_encipher_multiple_protostones() {
        let protostones = vec![
            Protostone {
                burn: Some(1u128),
                edicts: vec![],
                pointer: Some(3),
                refund: None,
                from: None,
                protocol_tag: 13,
                message: vec![],
            },
            Protostone {
                burn: Some(1u128),
                edicts: vec![],
                pointer: Some(2),
                refund: None,
                from: None,
                protocol_tag: 3,
                message: vec![100, 11, 112, 113, 114, 115, 116, 117, 118, 0, 0, 0, 0, 0, 0],
            },
        ];

        let protostone_enciphered = protostones.encipher().unwrap();

        let protostone_decipered = Protostone::decipher(&protostone_enciphered).unwrap();

        assert_eq!(protostones, protostone_decipered);
    }
}
