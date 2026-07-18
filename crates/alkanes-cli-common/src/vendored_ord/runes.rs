use serde::{Deserialize, Serialize};
use core::fmt::{self, Display, Formatter};
use core::str::FromStr;
use crate::serde_string;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryInto;
use alloc::string::ToString;

#[derive(Debug, PartialEq, Copy, Clone, Eq, PartialOrd, Ord, Default)]
pub struct Rune(pub u128);

serde_string!(Rune);

impl Rune {
    pub fn new(n: u128) -> Result<Self, u128> {
        if n >= 2u128.pow(128) {
            Err(n)
        } else {
            Ok(Self(n))
        }
    }
}

impl Display for Rune {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut n = self.0;
        if n == 0 {
            return write!(f, "A");
        }
        let mut s = String::new();
        while n > 0 {
            s.push(
                (b'A' + (n - 1) as u8 % 26) as char,
            );
            n = (n - 1) / 26;
        }
        for c in s.chars().rev() {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl FromStr for Rune {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut n = 0;
        for c in s.chars() {
            if !c.is_ascii_uppercase() {
                anyhow::bail!("invalid character in rune: {}", c);
            }
            n = n * 26 + (c as u128 - 'A' as u128 + 1);
        }
        Ok(Rune(n))
    }
}

#[derive(
    Debug, PartialEq, Copy, Clone, Hash, Eq, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct RuneId {
    pub block: u64,
    pub tx: u32,
}

impl Display for RuneId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.block, self.tx)
    }
}

impl FromStr for RuneId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (block, tx) = s
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid rune id: {}", s))?;
        Ok(Self {
            block: block.parse()?,
            tx: tx.parse()?,
        })
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default, Eq, PartialOrd, Ord)]
pub struct SpacedRune {
    pub rune: Rune,
    pub spacers: u32,
}

impl Display for SpacedRune {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let s = self.rune.to_string();
        for (i, c) in s.chars().enumerate() {
            write!(f, "{c}")?;
            if i < s.len() - 1 && (self.spacers >> i) & 1 != 0 {
                write!(f, "•")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct Edict {
    pub id: RuneId,
    pub amount: u128,
    pub output: u32,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default)]
pub struct Etching {
    pub divisibility: Option<u8>,
    pub premine: Option<u128>,
    pub rune: Option<Rune>,
    pub spacers: Option<u32>,
    pub symbol: Option<char>,
    pub terms: Option<Terms>,
    pub turbo: bool,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize, Default)]
pub struct Terms {
    pub amount: Option<u128>,
    pub cap: Option<u128>,
    pub height: (Option<u64>, Option<u64>),
    pub offset: (Option<u64>, Option<u64>),
}

#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Runestone {
    pub edicts: Vec<Edict>,
    pub etching: Option<Etching>,
    pub mint: Option<RuneId>,
    pub pointer: Option<u32>,
    pub protocol: Option<Vec<u128>>,
}

impl Runestone {
    pub fn encipher(&self) -> bitcoin::ScriptBuf {
        // This is a simplified encipher logic.
        // A full implementation would involve complex varint encoding.
        // For now, we just create a placeholder OP_RETURN.
        use bitcoin::blockdata::opcodes;
        use bitcoin::blockdata::script::Builder;

        let mut payload: Vec<u8> = Vec::new();
        // A real implementation would serialize the runestone fields into payload
        // using varint encoding.
        if let Some(protocol) = &self.protocol {
            // Simple serialization for now
            for val in protocol {
                payload.extend_from_slice(&val.to_le_bytes());
            }
        }
        
        Builder::new()
            .push_opcode(opcodes::all::OP_RETURN)
            .push_slice(b"R") // Magic number
            .push_slice::<&bitcoin::script::PushBytes>((&payload[..]).try_into().unwrap())
            .into_script()
    }
}