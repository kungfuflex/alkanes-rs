pub mod utils;
use crate::utils::{
    calc_merkle_root, decode_from_vec, extract_witness_payload, SchemaMerkleLeaf, SchemaMerkleProof,
};
use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_runtime::storage::StoragePointer;
use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::parcel::AlkaneTransferParcel;
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, ensure, Context, Result};
use bitcoin::{Address, Transaction};
use borsh::BorshDeserialize;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::{Artifact, Runestone};
use protorune_support::{protostone::Protostone, utils::consensus_decode};
use std::io::Cursor;
use std::sync::Arc;

#[derive(Default)]
pub struct MerkleDistributor(());

#[derive(MessageDispatch)]
enum MerkleDistributorMessage {
    #[opcode(0)]
    Initialize {
        input_alkane: AlkaneId,
        input_amount: u128,
        end_height: u128,
        root_first_half: u128,
        root_second_half: u128,
    },

    #[opcode(1)]
    Claim,

    #[opcode(2)]
    AuthCleanup { alkane: AlkaneId },

    #[opcode(50)]
    ForwardIncoming,

    #[opcode(51)]
    Donate,
}

pub fn overflow_error(v: Option<u128>) -> Result<u128> {
    v.ok_or("").map_err(|_| anyhow!("overflow error"))
}

pub fn sub_fees(v: u128) -> Result<u128> {
    Ok(overflow_error(v.checked_mul(997))? / 1000)
}

// storage
impl MerkleDistributor {
    pub fn root_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/root")
    }

    pub fn set_root(&self, v: Vec<u8>) {
        self.root_pointer().set(Arc::new(v))
    }

    pub fn end_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/end_height")
    }

    pub fn set_end_height(&self, v: u128) {
        self.end_height_pointer().set_value::<u128>(v);
    }

    pub fn end_height(&self) -> u128 {
        self.end_height_pointer().get_value::<u128>()
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

    fn get_used_leaf_pointer(&self, leaf_bytes: &Vec<u8>) -> StoragePointer {
        StoragePointer::from_keyword("/used").select(leaf_bytes)
    }
}

impl MerkleDistributor {
    #[cfg(not(any(feature = "mainnet", feature = "regtest", feature = "signet",)))]
    pub fn get_network(&self) -> bitcoin::Network {
        bitcoin::Network::Regtest
    }
    #[cfg(feature = "regtest")]
    pub fn get_network(&self) -> bitcoin::Network {
        bitcoin::Network::Regtest
    }
    #[cfg(feature = "signet")]
    pub fn get_network(&self) -> bitcoin::Network {
        bitcoin::Network::Signet
    }
    #[cfg(feature = "mainnet")]
    pub fn get_network(&self) -> bitcoin::Network {
        bitcoin::Network::Bitcoin
    }
    fn validate_proof(&self, proof: &SchemaMerkleProof) -> Result<()> {
        let merkle_root = self.root()?;
        let airdrop_end_height = self.end_height();

        let root_from_proof = calc_merkle_root(&proof.leaf, &proof.proofs);
        ensure!(merkle_root == root_from_proof, "Proof invalid");
        ensure!(self.height() as u128 <= airdrop_end_height, "Expired claim");

        Ok(())
    }
    pub fn validate_protostone_tx(
        &self,
        ctx: &alkanes_support::context::Context,
        tx: &Transaction,
    ) -> Result<()> {
        let runestone = match Runestone::decipher(&tx) {
            Some(Artifact::Runestone(r)) => r,
            _ => return Err(anyhow!("transaction does not contain a runestone")),
        };

        let protostones = Protostone::from_runestone(&runestone)
            .map_err(|e| anyhow!("failed to parse protostone: {e}"))?;

        let pm_index =
            ctx.vout
                .checked_sub(tx.output.len() as u32 + 1)
                .ok_or_else(|| anyhow!("vout is not a protomessage index"))? as usize;

        let message = protostones
            .get(pm_index)
            .ok_or_else(|| anyhow!("no protostone message at computed index"))?;

        if !message.edicts.is_empty() {
            return Err(anyhow!("protostone message must have zero edicts"));
        }

        let pointer = message
            .pointer
            .ok_or_else(|| anyhow!("protostone message has no pointer"))?;

        if pointer as usize >= tx.output.len() {
            return Err(anyhow!(
                "pointer index {pointer} points outside real user outputs"
            ));
        }

        if pointer != 0 {
            return Err(anyhow!("pointer must be set to 0! found {pointer}"));
        }

        Ok(())
    }
    pub fn verify_output(&self) -> Result<u128> {
        let ctx = self.context()?;
        let tx = self.transaction_object()?;

        self.validate_protostone_tx(&ctx, &tx)?;

        let witness_payload = match extract_witness_payload(&tx) {
            Some(bytes) => bytes,
            None => return Err(anyhow!("MERKLE DISTRIBUTOR: Failed to decode tx")),
        };

        let merkle_proof = decode_from_vec!(witness_payload, SchemaMerkleProof)
            .context("MERKLE DISTRIBUTOR: Failed to decode merkle proof from witness data")?;

        self.validate_proof(&merkle_proof)?;

        let mut ptr_used_leaf = self.get_used_leaf_pointer(&merkle_proof.leaf);
        let used_leaf_check = ptr_used_leaf.get_value::<u8>();

        ensure!(
            used_leaf_check == 0u8,
            "MERKLE DISTRIBUTOR: This leaf has already been used to claim"
        );

        let leaf = decode_from_vec!(merkle_proof.leaf, SchemaMerkleLeaf)?;

        let caller_script_pub_key = tx
            .tx_out(0)
            .context("MERKLE DISTRIBUTOR: vout #0 not present")?
            .clone()
            .script_pubkey;

        let tx_address = Address::from_script(&caller_script_pub_key, self.get_network())?;
        ensure!(
            tx_address.to_string() == leaf.address,
            "MERKLE DISTRIBUTOR: vout #0 doesnt contain the address in merkle proof"
        );

        ptr_used_leaf.set_value(1u8);
        Ok(leaf.amount)
    }

    fn _return_leftovers(
        &self,
        input_alkane: AlkaneId,
        input_amount: u128,
        input_alkanes: AlkaneTransferParcel,
    ) -> Result<CallResponse> {
        let mut response = CallResponse::default();
        for id in input_alkanes.0 {
            let value = if id.id == input_alkane {
                id.value - input_amount
            } else {
                id.value
            };
            response.alkanes.pay(AlkaneTransfer {
                id: id.id,
                value: value,
            });
        }
        Ok(response)
    }

    fn initialize(
        &self,
        input_alkane: AlkaneId,
        input_amount: u128,
        end_height: u128,
        root_first_half: u128,
        root_second_half: u128,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let myself = context.myself;
        if self.balance(&myself, &input_alkane) < input_amount {
            return Err(anyhow!(
                "user specified input amount is greater than actual input amount"
            ));
        }
        self.set_alkane(input_alkane.clone());
        self.set_end_height(end_height);

        let root = (&[root_first_half, root_second_half])
            .to_vec()
            .into_iter()
            .fold(Vec::<u8>::new(), |mut r, v| {
                r.extend(&v.to_le_bytes());
                r
            });
        self.set_root(root);

        let mut response =
            self._return_leftovers(input_alkane, input_amount, context.incoming_alkanes)?;
        response.alkanes.pay(self.deploy_self_auth_token(5)?);
        Ok(response)
    }

    fn claim(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(AlkaneTransfer {
            value: self.verify_output()?,
            id: self.alkane()?,
        });

        Ok(response)
    }

    fn auth_cleanup(&self, alkane: AlkaneId) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.alkanes.0.push(AlkaneTransfer {
            value: self.balance(&context.myself, &alkane),
            id: alkane,
        });

        Ok(response)
    }

    fn forward_incoming(&self) -> Result<CallResponse> {
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn donate(&self) -> Result<CallResponse> {
        Ok(CallResponse::default())
    }
}

impl AlkaneResponder for MerkleDistributor {}
impl AuthenticatedResponder for MerkleDistributor {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for MerkleDistributor {
        type Message = MerkleDistributorMessage;
    }
}
