use crate::gz::compress;
use {
    bitcoin::blockdata::{
        constants::MAX_SCRIPT_ELEMENT_SIZE,
        opcodes,
        script::{
            Instruction::{self, Op, PushBytes},
            Instructions,
        },
        witness::Witness,
    },
    bitcoin::psbt,
    bitcoin::script,
    bitcoin::script::Script,
    bitcoin::hashes::{sha256, Hash},
    bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey},
    bitcoin::taproot::{ControlBlock, LeafVersion, TaprootBuilder, TaprootSpendInfo},
    bitcoin::transaction::Transaction,
    bitcoin::Address,
    bitcoin::Network,
    bitcoin::TxOut,
    serde::{Deserialize, Serialize},
    std::iter::Peekable,
};

pub(crate) const PROTOCOL_ID: [u8; 3] = *b"BIN";
pub(crate) const BODY_TAG: [u8; 0] = [];

/// Maximum size, in bytes, of a single witness stack element / data push under
/// BIP-110 ("Reduced Data Temporary Softfork"). Hashlock preimages are chunked to
/// this bound so a reveal stays consensus-valid if BIP-110 ever activates.
pub const HASHLOCK_MAX_ELEMENT_SIZE: usize = 256;

/// BIP-341 NUMS ("nothing-up-my-sleeve") x-only point. Used as the unspendable
/// taproot internal key for hashlock commits so the only spend path is the leaf.
pub(crate) const NUMS_INTERNAL_KEY: [u8; 32] = [
    0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e,
    0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
];

pub type Result<T> = std::result::Result<T, script::Error>;
pub type RawEnvelope = Envelope<Vec<Vec<u8>>>;

#[derive(Default, PartialEq, Clone, Serialize, Deserialize, Debug, Eq)]
pub struct Envelope<T> {
    pub input: u32,
    pub offset: u32,
    pub payload: T,
    pub pushnum: bool,
    pub stutter: bool,
}

impl From<Vec<u8>> for RawEnvelope {
    fn from(v: Vec<u8>) -> RawEnvelope {
        RawEnvelope {
            input: 0,
            offset: 0,
            payload: v
                .chunks(MAX_SCRIPT_ELEMENT_SIZE)
                .into_iter()
                .map(|v| v.to_vec())
                .collect::<Vec<Vec<u8>>>(),
            pushnum: false,
            stutter: false,
        }
    }
}

impl RawEnvelope {
    pub fn from_transaction(transaction: &Transaction) -> Vec<Self> {
        let mut envelopes = Vec::new();

        for (i, input) in transaction.input.iter().enumerate() {
            if let Some(tapscript) = input.witness.tapscript() {
                if let Ok(input_envelopes) = Self::from_tapscript(tapscript, i) {
                    envelopes.extend(input_envelopes);
                }
            }
        }

        envelopes
    }

    fn from_tapscript(tapscript: &Script, input: usize) -> Result<Vec<Self>> {
        let mut envelopes = Vec::new();

        let mut instructions = tapscript.instructions().peekable();

        let mut stuttered = false;
        while let Some(instruction) = instructions.next().transpose()? {
            if instruction == PushBytes((&[]).into()) {
                let (stutter, envelope) =
                    Self::from_instructions(&mut instructions, input, envelopes.len(), stuttered)?;
                if let Some(envelope) = envelope {
                    envelopes.push(envelope);
                } else {
                    stuttered = stutter;
                }
            }
        }

        Ok(envelopes)
    }

    fn accept(instructions: &mut Peekable<Instructions>, instruction: Instruction) -> Result<bool> {
        if instructions.peek() == Some(&Ok(instruction)) {
            instructions.next().transpose()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn append_reveal_script(
        &self,
        mut builder: script::Builder,
        should_compress: bool,
    ) -> script::ScriptBuf {
        builder = builder
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(PROTOCOL_ID);

        builder = builder.push_slice(BODY_TAG);
        let mut payload = self
            .payload
            .clone()
            .into_iter()
            .flatten()
            .collect::<Vec<u8>>();
        if should_compress {
            payload = compress(payload).unwrap();
        }

        for chunk in payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
            builder = builder.push_slice::<&script::PushBytes>(chunk.try_into().unwrap());
        }
        builder.push_opcode(opcodes::all::OP_ENDIF).into_script()
    }
    pub fn to_witness(&self, should_compress: bool) -> Witness {
        let builder = script::Builder::new();

        let script = self.append_reveal_script(builder, should_compress);

        let mut witness = Witness::new();
        witness.push(script);
        witness.push([]);
        witness
    }

    /// Build a BIP-110-resistant "hashlock" reveal leaf for an already-compressed
    /// payload, plus the ordered preimages (preimage `j` is the SHA256 preimage of
    /// the `j`-th gate, in script order). The leaf is
    /// `(OP_SHA256 <h> OP_EQUALVERIFY){n} <reveal_key> OP_CHECKSIG` — an HTLC-shaped
    /// tapscript with no OP_IF and every push <= 256 bytes. No protocol marker is
    /// embedded; the indexer recognises it structurally (see `crate::witness`).
    pub fn hashlock_reveal_script(
        compressed_payload: &[u8],
        reveal_key: &XOnlyPublicKey,
    ) -> (script::ScriptBuf, Vec<Vec<u8>>) {
        let preimages: Vec<Vec<u8>> = compressed_payload
            .chunks(HASHLOCK_MAX_ELEMENT_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        let mut builder = script::Builder::new();
        for preimage in &preimages {
            let hash = sha256::Hash::hash(preimage);
            builder = builder
                .push_opcode(opcodes::all::OP_SHA256)
                .push_slice::<&script::PushBytes>(
                    hash.as_byte_array().as_slice().try_into().unwrap(),
                )
                .push_opcode(opcodes::all::OP_EQUALVERIFY);
        }
        builder = builder
            .push_slice::<&script::PushBytes>(
                reveal_key.serialize().as_slice().try_into().unwrap(),
            )
            .push_opcode(opcodes::all::OP_CHECKSIG);
        (builder.into_script(), preimages)
    }

    pub(crate) fn nums_internal_key() -> XOnlyPublicKey {
        XOnlyPublicKey::from_slice(&NUMS_INTERNAL_KEY).expect("valid NUMS x-only key")
    }

    /// Construct a complete Taproot script-path witness carrying `payload` via the
    /// hashlock envelope. Mirrors `to_witness`: `should_compress` gzip-compresses the
    /// payload first (the runtime expects a gzip stream). The signature slot is left
    /// empty — sufficient for indexing/parsing; a real broadcaster must re-sign over
    /// the taproot sighash.
    pub fn hashlock_witness(payload: Vec<u8>, should_compress: bool) -> Result<Witness> {
        let compressed = if should_compress {
            compress(payload).map_err(|_| script::Error::EarlyEndOfScript)?
        } else {
            payload
        };
        let internal_key = Self::nums_internal_key();
        // Disguise/spend key; not required to be spendable for indexing purposes.
        let reveal_key = internal_key;
        let (leaf, preimages) = Self::hashlock_reveal_script(&compressed, &reveal_key);

        let secp = Secp256k1::new();
        let spend_info = TaprootBuilder::new()
            .add_leaf(0, leaf.clone())
            .map_err(|_| script::Error::EarlyEndOfScript)?
            .finalize(&secp, internal_key)
            .map_err(|_| script::Error::EarlyEndOfScript)?;
        let control_block = spend_info
            .control_block(&(leaf.clone(), LeafVersion::TapScript))
            .ok_or(script::Error::EarlyEndOfScript)?;

        let mut witness = Witness::new();
        // Stack inputs, bottom-first: the signature, then the preimages in reverse
        // script order (the LIFO stack feeds the first SHA256 gate from the top).
        witness.push([]); // empty signature placeholder
        for preimage in preimages.iter().rev() {
            witness.push(preimage);
        }
        witness.push(leaf.as_bytes());
        witness.push(control_block.serialize());
        Ok(witness)
    }

    fn from_instructions(
        instructions: &mut Peekable<Instructions>,
        input: usize,
        offset: usize,
        stutter: bool,
    ) -> Result<(bool, Option<Self>)> {
        if !Self::accept(instructions, Op(opcodes::all::OP_IF))? {
            let stutter = instructions.peek() == Some(&Ok(PushBytes((&[]).into())));
            return Ok((stutter, None));
        }

        if !Self::accept(instructions, PushBytes((&PROTOCOL_ID).into()))? {
            let stutter = instructions.peek() == Some(&Ok(PushBytes((&[]).into())));
            return Ok((stutter, None));
        }

        let mut pushnum = false;

        let mut payload = Vec::new();

        loop {
            match instructions.next().transpose()? {
                None => return Ok((false, None)),
                Some(Op(opcodes::all::OP_ENDIF)) => {
                    return Ok((
                        false,
                        Some(Envelope {
                            input: input.try_into().unwrap(),
                            offset: offset.try_into().unwrap(),
                            payload,
                            pushnum,
                            stutter,
                        }),
                    ));
                }
                Some(Op(opcodes::all::OP_PUSHNUM_NEG1)) => {
                    pushnum = true;
                    payload.push(vec![0x81]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_1)) => {
                    pushnum = true;
                    payload.push(vec![1]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_2)) => {
                    pushnum = true;
                    payload.push(vec![2]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_3)) => {
                    pushnum = true;
                    payload.push(vec![3]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_4)) => {
                    pushnum = true;
                    payload.push(vec![4]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_5)) => {
                    pushnum = true;
                    payload.push(vec![5]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_6)) => {
                    pushnum = true;
                    payload.push(vec![6]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_7)) => {
                    pushnum = true;
                    payload.push(vec![7]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_8)) => {
                    pushnum = true;
                    payload.push(vec![8]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_9)) => {
                    pushnum = true;
                    payload.push(vec![9]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_10)) => {
                    pushnum = true;
                    payload.push(vec![10]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_11)) => {
                    pushnum = true;
                    payload.push(vec![11]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_12)) => {
                    pushnum = true;
                    payload.push(vec![12]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_13)) => {
                    pushnum = true;
                    payload.push(vec![13]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_14)) => {
                    pushnum = true;
                    payload.push(vec![14]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_15)) => {
                    pushnum = true;
                    payload.push(vec![15]);
                }
                Some(Op(opcodes::all::OP_PUSHNUM_16)) => {
                    pushnum = true;
                    payload.push(vec![16]);
                }
                Some(PushBytes(push)) => {
                    payload.push(push.as_bytes().to_vec());
                }
                Some(_) => return Ok((false, None)),
            }
        }
    }

    pub fn to_taproot_spend_info(&self, internal_key: XOnlyPublicKey) -> Result<TaprootSpendInfo> {
        let secp = Secp256k1::new();
        let builder = script::Builder::new();
        let reveal_script = self.append_reveal_script(builder, true);

        let taproot_spend_info = TaprootBuilder::new()
            .add_leaf(0, reveal_script)
            .map_err(|_| script::Error::EarlyEndOfScript)?
            .finalize(&secp, internal_key)
            .map_err(|_| script::Error::EarlyEndOfScript)?;

        Ok(taproot_spend_info)
    }

    pub fn to_control_block(&self, internal_key: XOnlyPublicKey) -> Result<ControlBlock> {
        let taproot_spend_info = self.to_taproot_spend_info(internal_key)?;
        let builder = script::Builder::new();
        let reveal_script = self.append_reveal_script(builder, true);

        taproot_spend_info
            .control_block(&(reveal_script, LeafVersion::TapScript))
            .ok_or(script::Error::EarlyEndOfScript)
    }

    pub fn prepare_psbt_input(
        &self,
        psbt_input: &mut psbt::Input,
        internal_key: XOnlyPublicKey,
        witness_utxo: TxOut,
    ) -> Result<()> {
        let control_block = self.to_control_block(internal_key)?;
        let builder = script::Builder::new();
        let reveal_script = self.append_reveal_script(builder, true);

        psbt_input.witness_utxo = Some(witness_utxo);
        psbt_input.tap_internal_key = Some(internal_key);
        psbt_input
            .tap_scripts
            .insert(control_block, (reveal_script, LeafVersion::TapScript));

        Ok(())
    }

    pub fn to_commit_address(
        &self,
        network: Network,
        internal_key: XOnlyPublicKey,
    ) -> Result<Address> {
        let secp = Secp256k1::new();
        let taproot_spend_info = self.to_taproot_spend_info(internal_key)?;

        let address = Address::p2tr(
            &secp,
            internal_key,
            taproot_spend_info.merkle_root(),
            network,
        );

        Ok(address)
    }
}
