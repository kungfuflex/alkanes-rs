use crate::balance_sheet::OutgoingRunes;
use crate::{
    message::{MessageContext, MessageContextParcel},
    protoburn::{Protoburn, Protoburns},
};
use anyhow::Result;
use bitcoin::{Block, Transaction, Txid};
use metashrew::index_pointer::{AtomicPointer, IndexPointer};
use ordinals::Runestone;
use protorune_support::{
    balance_sheet::BalanceSheet,
    protostone::{split_bytes, Protostone},
    rune_transfer::{refund_to_refund_pointer, RuneTransfer},
    utils::encode_varint_list,
};
use std::collections::{HashMap, HashSet};

use metashrew::{println, stdio::stdout};
use std::fmt::Write;

static mut PROTOCOLS: Option<HashSet<u128>> = None;

#[allow(static_mut_refs)]
pub fn initialized_protocol_index() -> Result<()> {
    unsafe { PROTOCOLS = Some(HashSet::new()) }
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
    fn process_message<T: MessageContext>(
        &self,
        atomic: &mut AtomicPointer,
        transaction: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        _runestone_output_index: u32,
        protomessage_vout: u32,
        balances_by_output: &mut HashMap<u32, BalanceSheet<AtomicPointer>>,
        default_output: u32,
        num_protostones: usize,
    ) -> Result<()>;
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
        balances_by_output: &mut HashMap<u32, BalanceSheet<AtomicPointer>>,
        default_output: u32,
        num_protostones: usize,
    ) -> Result<()> {
        // Validate output indexes and protomessage_vout
        let num_outputs = transaction.output.len();
        let pointer = self.pointer.unwrap_or(default_output);
        let refund_pointer = self.refund.unwrap_or(default_output);

        // Ensure pointers are valid transaction outputs
        if pointer > (num_outputs + num_protostones) as u32
            || refund_pointer > (num_outputs + num_protostones) as u32
        {
            return Err(anyhow::anyhow!("Invalid output pointer"));
        }

        // Log the Bitcoin address that can spend the output pointed to by the "pointer" field
        if pointer < num_outputs as u32 {
            if let Some(address) = protorune_support::network::to_address_str(
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
            if let Some(address) = protorune_support::network::to_address_str(
                &transaction.output[refund_pointer as usize].script_pubkey,
            ) {
                println!(
                    "Protostone refund_pointer ({}) points to Bitcoin address: {}",
                    refund_pointer, address
                );
            }
        }

        // Validate protomessage vout to prevent overflow attacks
        // Add a reasonable maximum based on transaction size
        let max_virtual_vout = num_outputs + 100; // Adjust limit as needed
        if protomessage_vout >= max_virtual_vout as u32 {
            return Err(anyhow::anyhow!("Protomessage vout exceeds maximum allowed"));
        }
        let initial_sheet = balances_by_output
            .get(&protomessage_vout)
            .map(|v| v.clone())
            .unwrap_or_else(|| BalanceSheet::default());

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
                match values.reconcile(
                    atomic,
                    balances_by_output,
                    protomessage_vout,
                    pointer,
                    refund_pointer,
                ) {
                    Ok(_) => atomic.commit(),
                    Err(e) => {
                        println!("Got error inside reconcile! {:?} \n\n", e);
                        println!("Refunding to refund_pointer: {}", refund_pointer);

                        // Log the Bitcoin address again to make it clear this is the refund address being used
                        if refund_pointer < num_outputs as u32 {
                            if let Some(address) = protorune_support::network::to_address_str(
                                &transaction.output[refund_pointer as usize].script_pubkey,
                            ) {
                                println!("RECONCILE ERROR REFUND: Protostone refund_pointer ({}) points to Bitcoin address: {}", refund_pointer, address);
                            }
                        }

                        refund_to_refund_pointer(
                            balances_by_output,
                            protomessage_vout,
                            refund_pointer,
                        );
                        atomic.rollback()
                    }
                }
            }
            Err(e) => {
                println!("Alkanes message reverted with error: {:?}", e);
                println!("Refunding to refund_pointer: {}", refund_pointer);

                // Log the Bitcoin address again to make it clear this is the refund address being used
                if refund_pointer < num_outputs as u32 {
                    if let Some(address) = protorune_support::network::to_address_str(
                        &transaction.output[refund_pointer as usize].script_pubkey,
                    ) {
                        println!(
                            "REFUND: Protostone refund_pointer ({}) points to Bitcoin address: {}",
                            refund_pointer, address
                        );
                    }
                }

                refund_to_refund_pointer(balances_by_output, protomessage_vout, refund_pointer);
                atomic.rollback();
            }
        }
        Ok(())
    }
}

pub trait Protostones {
    fn burns(&self) -> Result<Vec<Protoburn>>;
    fn process_burns(
        &self,
        atomic: &mut AtomicPointer,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &HashMap<u32, BalanceSheet<AtomicPointer>>,
        proto_balances_by_output: &mut HashMap<u32, BalanceSheet<AtomicPointer>>,
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
        balances_by_output: &HashMap<u32, BalanceSheet<AtomicPointer>>,
        proto_balances_by_output: &mut HashMap<u32, BalanceSheet<AtomicPointer>>,
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
