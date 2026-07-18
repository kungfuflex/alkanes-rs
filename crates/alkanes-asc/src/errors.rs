// Copyright 2024 The Deezel Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Error types.

extern crate alloc;
use alloc::string::{String, ToString};

#[cfg(feature = "std")]
use std::io;

/// The `Result` type for this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// The error type for this crate.
#[derive(Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum Error {
    /// An I/O error.
    #[cfg(feature = "std")]
    #[error("IO error")]
    Io(#[from] io::Error),
    /// An error from the `nom` parser combinator library.
    #[error("Nom parser error: {0}")]
    Nom(String),
    /// An error from the `base64` library.
    #[error("Base64 decode error")]
    Base64(#[from] base64::DecodeError),
    /// An invalid input error.
    #[error("Invalid input")]
    InvalidInput,
    /// A custom error.
    #[error("{0}")]
    Other(String),
}


#[cfg(feature = "std")]

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Error::Nom(err.to_string())
    }
}


impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}