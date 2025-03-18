use alkanes_runtime::storage::StoragePointer;
use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{
    id::AlkaneId,
    parcel::AlkaneTransfer,
    response::CallResponse,
    utils::{shift_bytes32_or_err, shift_or_err},
    witness::find_witness_payload,
};
use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::{
    compat::{to_arraybuffer_layout, to_passback_ptr},
    utils::{consume_exact, consume_sized_int, consume_to_end},
};
use ordinals::{Artifact, Runestone};
use protorune_support::{protostone::Protostone, utils::consensus_decode};
use rs_merkle::{algorithms::Sha256, Hasher, MerkleProof};
use std::io::Cursor;
use std::sync::Arc;

#[derive(Default)]
pub struct MerkleDistributor(());

#[derive(MessageDispatch)]
enum MerkleDistributorMessage {
    #[opcode(0)]
    Initialize { length: u128, root_bytes: u128 },

    #[opcode(1)]
    Claim,
}

pub fn overflow_error(v: Option<u128>) -> Result<u128> {
    v.ok_or("").map_err(|_| anyhow!("overflow error"))
}

pub fn sub_fees(v: u128) -> Result<u128> {
    Ok(overflow_error(v.checked_mul(997))? / 1000)
}

impl MerkleDistributor {
    pub fn verify_output(&self, vout: u32) -> Result<u128> {
        let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(self.transaction()))?;
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(&tx) {
            let protostones = Protostone::from_runestone(runestone)?;
            let message = &protostones[(vout as usize) - tx.output.len() - 1];
            if message.edicts.len() != 0 {
                panic!("message cannot contain edicts, only a pointer")
            }
            let pointer = message
                .pointer
                .ok_or("")
                .map_err(|_| anyhow!("no pointer in message"))?;
            if pointer as usize >= tx.output.len() {
                panic!("pointer cannot be a protomessage");
            }
            let mut cursor: Cursor<Vec<u8>> =
                Cursor::<Vec<u8>>::new(find_witness_payload(&tx, 0).ok_or("").map_err(|_| {
                    anyhow!("merkle-distributor: witness envelope at index 0 does not contain data")
                })?);
            let leaf = consume_exact(&mut cursor, 40)?;
            let leaf_hash = Sha256::hash(&leaf);
            let proof = consume_to_end(&mut cursor)?;
            let mut leaf_cursor = Cursor::new(leaf.clone());
            let p2sh = consume_exact(&mut leaf_cursor, 20)?;
            let index = consume_sized_int::<u32>(&mut leaf_cursor)? as usize;
            let amount = consume_sized_int::<u128>(&mut leaf_cursor)?;
            if MerkleProof::<Sha256>::try_from(proof)?.verify(
                self.root()?,
                &[index],
                &[leaf_hash],
                self.length(),
            ) {
                if tx.output[(vout as usize) as usize]
                    .script_pubkey
                    .clone()
                    .into_bytes()
                    .to_vec()
                    != p2sh
                {
                    Err(anyhow!("spendable output created does not match proof"))
                } else {
                    Ok(amount)
                }
            } else {
                Err(anyhow!("proof verification failure"))
            }
        } else {
            Err(anyhow!("runestone decipher failed"))
        }
    }

    pub fn length_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/length")
    }

    pub fn root_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/root")
    }

    pub fn set_length(&self, v: usize) {
        self.length_pointer().set_value::<usize>(v);
    }

    pub fn set_root(&self, v: Vec<u8>) {
        self.root_pointer().set(Arc::new(v))
    }

    pub fn length(&self) -> usize {
        self.length_pointer().get_value::<usize>()
    }

    pub fn root(&self) -> Result<[u8; 32]> {
        let root_vec: Vec<u8> = self.root_pointer().get().as_ref().clone();
        let root_bytes: &[u8] = root_vec.as_ref();
        root_bytes
            .try_into()
            .map_err(|_| anyhow!("root bytes in storage are not of length 32"))
    }

    pub fn alkane_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/alkane")
    }

    pub fn alkane(&self) -> Result<AlkaneId> {
        Ok(self.alkane_pointer().get().as_ref().clone().try_into()?)
    }

    pub fn set_alkane(&self, v: AlkaneId) {
        self.alkane_pointer().set(Arc::<Vec<u8>>::new(v.into()));
    }

    fn initialize(&self, length: u128, root_bytes: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut inputs = context.inputs.clone();

        let mut pointer = StoragePointer::from_keyword("/initialized");
        if pointer.get().len() == 0 {
            pointer.set(Arc::new(vec![0x01]));
            if context.incoming_alkanes.0.len() != 1 {
                panic!("must send 1 alkane to lock for distribution");
            }
            self.set_alkane(context.incoming_alkanes.0[0].id.clone());

            // Extract the remaining parameters from inputs
            self.set_length(length.try_into().unwrap());
            self.set_root(shift_bytes32_or_err(&mut inputs)?);

            Ok(CallResponse::default())
        } else {
            Err(anyhow!("already initialized"))
        }
    }

    fn claim(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(AlkaneTransfer {
            value: self.verify_output(context.vout)?,
            id: self.alkane()?,
        });

        Ok(response)
    }
}

impl AlkaneResponder for MerkleDistributor {
    fn execute(&self) -> Result<CallResponse> {
        // The opcode extraction and dispatch logic is now handled by the declare_alkane macro
        // This method is still required by the AlkaneResponder trait, but we can just return an error
        // indicating that it should not be called directly
        Err(anyhow!(
            "This method should not be called directly. Use the declare_alkane macro instead."
        ))
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for MerkleDistributor {
        type Message = MerkleDistributorMessage;
    }
}
