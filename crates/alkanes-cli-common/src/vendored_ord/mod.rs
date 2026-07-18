//! Vendored code from the `ord` crate to avoid dependency issues.
//! This module contains types related to Ordinals and Runes.

pub mod inscription_id;
pub mod runes;

pub use inscription_id::{
    InscriptionId,
    ParseError as InscriptionIdParseError,
};

pub use runes::{
    Edict,
    Etching,
    Rune,
    RuneId,
    Runestone,
    SpacedRune,
    Terms,
};