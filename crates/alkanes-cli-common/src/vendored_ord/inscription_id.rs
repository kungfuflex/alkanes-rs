use bitcoin::{hashes::Hash, Txid};
use serde::{Deserialize, Deserializer, Serialize};
use core::fmt::{self, Display, Formatter};
use core::str::FromStr;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::string::ToString;

// Helper macro for serde
#[macro_export]
macro_rules! serde_string {
    ($type:ty) => {
        impl Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::from_str(&String::deserialize(deserializer)?)
                    .map_err(|err| serde::de::Error::custom(err.to_string()))
            }
        }
    };
}

// Custom implementation of DeserializeFromStr and SerializeDisplay to avoid dependency
pub trait DeserializeFromStr: Sized + FromStr {
    fn deserialize_from_str<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
        Self::Err: Display;
}

impl<T> DeserializeFromStr for T
where
    T: FromStr,
    T::Err: Display,
{
    fn deserialize_from_str<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        T::from_str(&s).map_err(serde::de::Error::custom)
    }
}

pub trait SerializeDisplay: Sized + Display {
    fn serialize_display<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer;
}

impl<T: Display> SerializeDisplay for T {
    fn serialize_display<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}


#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq, PartialOrd, Ord)]
pub struct InscriptionId {
  pub txid: Txid,
  pub index: u32,
}

serde_string!(InscriptionId);

impl Default for InscriptionId {
  fn default() -> Self {
    Self {
      txid: Txid::all_zeros(),
      index: 0,
    }
  }
}

#[allow(dead_code)]
impl InscriptionId {
  pub(crate) fn from_value(value: &[u8]) -> Option<Self> {
    if value.len() < Txid::LEN {
      return None;
    }

    if value.len() > Txid::LEN + 4 {
      return None;
    }

    let (txid, index) = value.split_at(Txid::LEN);

    if let Some(last) = index.last() {
      // Accept fixed length encoding with 4 bytes (with potential trailing zeroes)
      // or variable length (no trailing zeroes)
      if index.len() != 4 && *last == 0 {
        return None;
      }
    }

    let txid = Txid::from_slice(txid).unwrap();

    let index = [
      index.first().copied().unwrap_or_default(),
      index.get(1).copied().unwrap_or_default(),
      index.get(2).copied().unwrap_or_default(),
      index.get(3).copied().unwrap_or_default(),
    ];

    let index = u32::from_le_bytes(index);

    Some(Self { txid, index })
  }

  pub(crate) fn value(self) -> Vec<u8> {
    let index = self.index.to_le_bytes();
    let mut index_slice = index.as_slice();

    while index_slice.last().copied() == Some(0) {
      index_slice = &index_slice[0..index_slice.len() - 1];
    }

    self
      .txid
      .to_byte_array()
      .iter()
      .chain(index_slice)
      .copied()
      .collect()
  }
}

impl Display for InscriptionId {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}i{}", self.txid, self.index)
  }
}

#[derive(Debug)]
pub enum ParseError {
  Character(char),
  Length(usize),
  Separator(char),
  Txid(bitcoin::hashes::hex::HexToArrayError),
  Index(core::num::ParseIntError),
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

impl Display for ParseError {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    match self {
      Self::Character(c) => write!(f, "invalid character: '{c}'"),
      Self::Length(len) => write!(f, "invalid length: {len}"),
      Self::Separator(c) => write!(f, "invalid separator: `{c}`"),
      Self::Txid(err) => write!(f, "invalid txid: {err}"),
      Self::Index(err) => write!(f, "invalid index: {err}"),
    }
  }
}


impl FromStr for InscriptionId {
  type Err = ParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if let Some(char) = s.chars().find(|char| !char.is_ascii()) {
      return Err(ParseError::Character(char));
    }

    const TXID_LEN: usize = 64;
    const MIN_LEN: usize = TXID_LEN + 2;

    if s.len() < MIN_LEN {
      return Err(ParseError::Length(s.len()));
    }

    let txid = &s[..TXID_LEN];

    let separator = s.chars().nth(TXID_LEN).unwrap();

    if separator != 'i' {
      return Err(ParseError::Separator(separator));
    }

    let vout = &s[TXID_LEN + 1..];

    Ok(Self {
      txid: txid.parse().map_err(ParseError::Txid)?,
      index: vout.parse().map_err(ParseError::Index)?,
    })
  }
}