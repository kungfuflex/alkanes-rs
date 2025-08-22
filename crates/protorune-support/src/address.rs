use anyhow::{anyhow, Error, Result};
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::str::FromStr;

use bech32::primitives::hrp::Hrp;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Secp256k1, Verification};

use bitcoin::base58;
use bitcoin::blockdata::constants::MAX_SCRIPT_ELEMENT_SIZE;
use bitcoin::blockdata::script::witness_program::WitnessProgram;
use bitcoin::blockdata::script::witness_version::WitnessVersion;
use bitcoin::blockdata::script::{self, Script, ScriptBuf, ScriptHash};
use bitcoin::key::{TapTweak, TweakedPublicKey, UntweakedPublicKey};
use bitcoin::taproot::TapNodeHash;
use bitcoin::{PubkeyHash, PublicKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum AddressType {
    P2pkh,
    P2sh,
    P2wpkh,
    P2wsh,
    P2tr,
}

impl fmt::Display for AddressType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            AddressType::P2pkh => "p2pkh",
            AddressType::P2sh => "p2sh",
            AddressType::P2wpkh => "p2wpkh",
            AddressType::P2wsh => "p2wsh",
            AddressType::P2tr => "p2tr",
        })
    }
}

impl FromStr for AddressType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "p2pkh" => Ok(AddressType::P2pkh),
            "p2sh" => Ok(AddressType::P2sh),
            "p2wpkh" => Ok(AddressType::P2wpkh),
            "p2wsh" => Ok(AddressType::P2wsh),
            "p2tr" => Ok(AddressType::P2tr),
            _ => Err(anyhow!(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Payload {
    PubkeyHash(PubkeyHash),
    ScriptHash(ScriptHash),
    WitnessProgram(WitnessProgram),
}

impl Payload {
    pub fn from_script(script: &Script) -> Result<Payload> {
        Ok(if script.is_p2pkh() {
            let bytes = script.as_bytes()[3..23]
                .try_into()
                .expect("statically 20B long");
            Payload::PubkeyHash(PubkeyHash::from_byte_array(bytes))
        } else if script.is_p2sh() {
            let bytes = script.as_bytes()[2..22]
                .try_into()
                .expect("statically 20B long");
            Payload::ScriptHash(ScriptHash::from_byte_array(bytes))
        } else if script.is_witness_program() {
            let opcode = script
                .first_opcode()
                .expect("witness_version guarantees len() > 4");

            let witness_program = script.as_bytes()[2..].to_vec();

            let witness_program =
                WitnessProgram::new(WitnessVersion::try_from(opcode)?, &witness_program)?;
            Payload::WitnessProgram(witness_program)
        } else {
            return Err(anyhow!("unrecognized script"));
        })
    }

    pub fn script_pubkey(&self) -> ScriptBuf {
        match *self {
            Payload::PubkeyHash(ref hash) => ScriptBuf::new_p2pkh(hash),
            Payload::ScriptHash(ref hash) => ScriptBuf::new_p2sh(hash),
            Payload::WitnessProgram(ref prog) => ScriptBuf::new_witness_program(prog),
        }
    }

    pub fn matches_script_pubkey(&self, script: &Script) -> bool {
        match *self {
            Payload::PubkeyHash(ref hash) if script.is_p2pkh() => {
                &script.as_bytes()[3..23] == <PubkeyHash as AsRef<[u8; 20]>>::as_ref(hash)
            }
            Payload::ScriptHash(ref hash) if script.is_p2sh() => {
                &script.as_bytes()[2..22] == <ScriptHash as AsRef<[u8; 20]>>::as_ref(hash)
            }
            Payload::WitnessProgram(ref prog) if script.is_witness_program() => {
                &script.as_bytes()[2..] == prog.program().as_bytes()
            }
            Payload::PubkeyHash(_) | Payload::ScriptHash(_) | Payload::WitnessProgram(_) => false,
        }
    }

    #[inline]
    pub fn p2pkh(pk: &PublicKey) -> Payload {
        Payload::PubkeyHash(pk.pubkey_hash())
    }

    #[inline]
    pub fn p2sh(script: &Script) -> Result<Payload, Error> {
        if script.len() > MAX_SCRIPT_ELEMENT_SIZE {
            return Err(anyhow!("excessive script size"));
        }
        Ok(Payload::ScriptHash(script.script_hash()))
    }

    pub fn p2wpkh(pk: &PublicKey) -> Result<Payload> {
        let prog = WitnessProgram::new(
            WitnessVersion::V0,
            pk.wpubkey_hash()
                .map_err(|_| anyhow!("uncompressed public key"))?
                .as_ref(),
        )?;
        Ok(Payload::WitnessProgram(prog))
    }

    pub fn p2shwpkh(pk: &PublicKey) -> Result<Payload> {
        let builder = script::Builder::new().push_int(0).push_slice(
            pk.wpubkey_hash()
                .map_err(|_| anyhow!("uncompressed public key"))?,
        );

        Ok(Payload::ScriptHash(builder.into_script().script_hash()))
    }

    pub fn p2wsh(script: &Script) -> Payload {
        let prog = WitnessProgram::new(WitnessVersion::V0, script.wscript_hash().as_ref())
            .expect("wscript_hash has len 32 compatible with segwitv0");
        Payload::WitnessProgram(prog)
    }

    pub fn p2shwsh(script: &Script) -> Payload {
        let ws = script::Builder::new()
            .push_int(0)
            .push_slice(script.wscript_hash())
            .into_script();

        Payload::ScriptHash(ws.script_hash())
    }

    pub fn p2tr<C: Verification>(
        secp: &Secp256k1<C>,
        internal_key: UntweakedPublicKey,
        merkle_root: Option<TapNodeHash>,
    ) -> Payload {
        let (output_key, _parity) = internal_key.tap_tweak(secp, merkle_root);
        let prog = WitnessProgram::new(
            WitnessVersion::V1,
            &output_key.to_x_only_public_key().serialize(),
        )
        .expect("taproot output key has len 32 <= 40");
        Payload::WitnessProgram(prog)
    }

    pub fn p2tr_tweaked(output_key: TweakedPublicKey) -> Payload {
        let prog = WitnessProgram::new(
            WitnessVersion::V1,
            &output_key.to_x_only_public_key().serialize(),
        )
        .expect("taproot output key has len 32 <= 40");
        Payload::WitnessProgram(prog)
    }
}

#[derive(Debug)]
pub struct AddressEncoding<'a> {
    pub payload: &'a Payload,
    pub p2pkh_prefix: u8,
    pub p2sh_prefix: u8,
    pub hrp: Hrp,
}

impl<'a> fmt::Display for AddressEncoding<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.payload {
            Payload::PubkeyHash(hash) => {
                let mut prefixed = [0; 21];
                prefixed[0] = self.p2pkh_prefix;
                prefixed[1..].copy_from_slice(&hash[..]);
                base58::encode_check_to_fmt(fmt, &prefixed[..])
            }
            Payload::ScriptHash(hash) => {
                let mut prefixed = [0; 21];
                prefixed[0] = self.p2sh_prefix;
                prefixed[1..].copy_from_slice(&hash[..]);
                base58::encode_check_to_fmt(fmt, &prefixed[..])
            }
            Payload::WitnessProgram(witness_program) => {
                let hrp = self.hrp.clone();
                let version = witness_program.version().to_fe();
                let program = witness_program.program().as_bytes();

                if fmt.alternate() {
                    bech32::segwit::encode_upper_to_fmt_unchecked(fmt, hrp, version, program)
                } else {
                    bech32::segwit::encode_lower_to_fmt_unchecked(fmt, hrp, version, program)
                }
            }
        }
    }
}