use crate::balance_sheet::OutgoingRunes;
use crate::{
    message::{MessageContext, MessageContextParcel},
    protoburn::{Protoburn, Protoburns},
};
use anyhow::{anyhow, Result};
use bitcoin::{Address, Block, Network, Transaction, Txid};
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};
use ordinals::Runestone;
use protorune_support::{
    balance_sheet::BalanceSheet,
    protostone::{split_bytes, Protostone},
    rune_transfer::{refund_to_refund_pointer, RuneTransfer},
    utils::encode_varint_list,
};
use std::collections::{BTreeMap, BTreeSet};

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

pub trait MessageHandler<E: RuntimeEnvironment + Default + Clone> {
    fn process_message<T: MessageContext<E>>(
        &self,
        env: &mut E,
        atomic: &mut AtomicPointer<E>,
        transaction: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        _runestone_output_index: u32,
        protomessage_vout: u32,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        num_protostones: usize,
    ) -> Result<bool>;
}

pub trait MessageProcessor<E: RuntimeEnvironment + Default + Clone>: ToString {
    fn handle(
        &self,
        parcel: &MessageContextParcel<E>,
        env: &mut E,
    ) -> Result<(Vec<RuneTransfer>, BalanceSheet<E, AtomicPointer<E>>)>;
}
impl<E: RuntimeEnvironment + Default + Clone> MessageHandler<E> for Protostone {
    fn process_message<T: MessageContext<E>>(
        &self,
        env: &mut E,
        atomic: &mut AtomicPointer<E>,
        transaction: &Transaction,
        txindex: u32,
        block: &Block,
        height: u64,
        _runestone_output_index: u32,
        protomessage_vout: u32,
        balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        num_protostones: usize,
    ) -> Result<bool> {
        // Validate output indexes and protomessage_vout
        let num_outputs = transaction.output.len();
        let pointer = self.pointer.ok_or_else(|| anyhow!("Missing pointer"))?;
        let refund_pointer = self
            .refund
            .ok_or_else(|| anyhow!("Missing refund pointer"))?;

        // Ensure pointers are valid transaction outputs
        if pointer > (num_outputs + num_protostones) as u32
            || refund_pointer > (num_outputs + num_protostones) as u32
        {
            return Err(anyhow::anyhow!("Invalid output pointer"));
        }

        // Log the Bitcoin address that can spend the output pointed to by the "pointer" field
        if pointer < num_outputs as u32 {
            if let Ok(_address) = Address::from_script(
                &transaction.output[pointer as usize].script_pubkey, Network::Regtest
            ) {
            }
        }

        // Log the Bitcoin address that can spend the output pointed to by the "refund_pointer" field
        if refund_pointer < num_outputs as u32 {
            if let Ok(_address) = Address::from_script(
                &transaction.output[refund_pointer as usize].script_pubkey, Network::Regtest
            ) {
            }
        }

        // Validate protomessage vout to prevent overflow attacks
        // Add a reasonable maximum based on transaction size
        let max_virtual_vout = num_outputs + 100; // Adjust limit as needed
        if protomessage_vout >= max_virtual_vout as u32 {
            return Err(anyhow::anyhow!("Protomessage vout exceeds maximum allowed"));
        }
        let default_sheet = BalanceSheet::default();
        let initial_sheet = balances_by_output
            .get(&protomessage_vout)
            .map(|v| v)
            .unwrap_or(&default_sheet);

        // Create a nested atomic transaction for the entire message processing
        atomic.checkpoint();

        let parcel = MessageContextParcel::<E> {
            atomic: atomic.derive(&IndexPointer::default()),
            runes: RuneTransfer::from_balance_sheet(initial_sheet),
            transaction: transaction.clone(),
            block: block.clone(),
            height,
            vout: protomessage_vout,
            pointer,
            refund_pointer,
            calldata: self.message.iter().flat_map(|v| v.to_be_bytes()).collect(),
            txindex,
            runtime_balances: Box::new(balances_by_output.remove(&u32::MAX).unwrap_or_default()),
            sheets: Box::new(BalanceSheet::default()),
			_phantom: std::marker::PhantomData::<E>,
        };

        match T::handle(&parcel, env) {
            Ok(values) => {
                match values.reconcile(atomic, balances_by_output, protomessage_vout, pointer, env) {
                    Ok(_) => {
                        atomic.commit(env);
                        Ok(true)
                    }
                    Err(_e) => {

                        // Log the Bitcoin address again to make it clear this is the refund address being used
                        if refund_pointer < num_outputs as u32 {
                            if let Ok(_address) = Address::from_script(
                                &transaction.output[refund_pointer as usize].script_pubkey, Network::Regtest
                            ) {
                            }
                        }

                        refund_to_refund_pointer(
                            balances_by_output,
                            protomessage_vout,
                            refund_pointer,
                            env,
                        )?;
                        atomic.rollback();
                        Ok(false)
                    }
                }
            }
            Err(_e) => {

                // Log the Bitcoin address again to make it clear this is the refund address being used
                if refund_pointer < num_outputs as u32 {
                    if let Ok(_address) = Address::from_script(
                        &transaction.output[refund_pointer as usize].script_pubkey, Network::Regtest
                    ) {
                    }
                }

                refund_to_refund_pointer(balances_by_output, protomessage_vout, refund_pointer, env)?;
                atomic.rollback();

                Ok(false)
            }
        }
    }
}

pub trait ProtostoneEncoder<E: RuntimeEnvironment + Clone> {
    fn burns(&self) -> Result<Vec<Protoburn<E>>>;
    fn encipher(&self) -> Result<Vec<u128>>;
}

pub trait Protostones<E: RuntimeEnvironment + Clone> {
    fn process_burns(
        &self,
        env: &mut E,
        atomic: &mut AtomicPointer<E>,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()>;
}

impl<E: RuntimeEnvironment + Clone> ProtostoneEncoder<E> for Vec<Protostone> {
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
    fn burns(&self) -> Result<Vec<Protoburn<E>>> {
        Ok(self
            .into_iter()
            .filter(|stone| stone.burn.is_some())
            .map(|stone| Protoburn {
                tag: stone.burn.map(|v| v as u128),
                pointer: stone.pointer,
                from: stone.from.map(|v| vec![v]),
				_phantom: std::marker::PhantomData::<E>,
            })
            .collect())
    }
}

impl<E: RuntimeEnvironment + Clone> Protostones<E> for Vec<Protostone> {
    fn process_burns(
        &self,
        env: &mut E,
        atomic: &mut AtomicPointer<E>,
        runestone: &Runestone,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<E, AtomicPointer<E>>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()> {
        let mut burns = self.burns()?;
        burns.process(
            env,
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

    use super::*;

    #[test]
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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