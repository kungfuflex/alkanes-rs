//! String interner implementation for alkanes-collections.
//!
//! This module provides a unified interface that can use either:
//! - The built-in string interner implementation (detail.rs)
//! - The alkanes-string-interner crate for enhanced SPIR-V compatibility

pub mod detail;

// Re-export the main types from detail.rs
pub use self::detail::{StringInterner, StringInternerImpl};

// Also re-export from alkanes-string-interner if available
#[cfg(feature = "alkanes-string-interner")]
pub use alkanes_string_interner::{
    DefaultStringInterner as AlkanesStringInterner,
    StringInterner as AlkanesStringInternerGeneric,
    Symbol, DefaultSymbol,
    backend::{Backend, DefaultBackend},
};

// Define the main types and traits that the rest of the codebase expects
pub trait GetOrInternWithHint {
    /// Interns the given string with a hint about whether it likely exists.
    fn get_or_intern_with_hint<T>(&mut self, string: T, hint: InternHint) -> Sym
    where
        T: AsRef<str>;
}

/// Hints for string interning optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternHint {
    /// No hint provided.
    None,
    /// The string likely already exists in the interner.
    LikelyExists,
    /// The string is likely new to the interner.
    LikelyNew,
}

/// A symbol representing an interned string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sym(u32);

impl Sym {
    /// Creates a new symbol from a usize value.
    #[inline]
    pub fn from_usize(value: usize) -> Self {
        Self(value as u32)
    }

    /// Converts the symbol to a usize value.
    #[inline]
    pub fn into_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for Sym {
    #[inline]
    fn from(value: usize) -> Self {
        Self::from_usize(value)
    }
}

impl From<Sym> for usize {
    #[inline]
    fn from(sym: Sym) -> Self {
        sym.into_usize()
    }
}